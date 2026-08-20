//! Anthropic OAuth usage-endpoint fetcher + on-disk cache.
//!
//! Port of the essentials of `utils/usage-fetch.ts`. Scope:
//!  - Read `~/.claude/.credentials.json` -> `claudeAiOauth.accessToken`,
//!    with a macOS Keychain fallback (service `Claude Code-credentials`)
//!    that picks whichever source has the newer mtime/mdat.
//!  - GET https://api.anthropic.com/api/oauth/usage w/ Bearer token,
//!    honoring `HTTPS_PROXY` / `https_proxy` and `NO_PROXY` env vars.
//!  - Parse the flat five_hour/seven_day/seven_day_sonnet/seven_day_opus
//!    buckets, plus the `limits[]` fallback for accounts migrated to the
//!    newer response shape (#503 in ccstatusline).
//!  - Cache the result at `%LOCALAPPDATA%\glassline\usage.json` (Windows) or
//!    `$XDG_CACHE_HOME/glassline/usage.json` with a per-outcome TTL.
//!
//! # Cross-process safety
//!
//! Concurrent glassline invocations coordinate via
//! `%LOCALAPPDATA%\glassline\usage.lock` (Windows) or
//! `$XDG_CACHE_HOME/glassline/usage.lock` (unix). At most one process
//! hits `/api/oauth/usage` per `LOCK_MAX_AGE_MS` (30 s). On contention
//! we return whatever's in the cache — even if stale — rather than
//! block the render pipeline.
//!
//! # Module layout
//!
//! - `credentials` — file-based OAuth token resolution + `resolve_access_token`.
//! - `keychain` — macOS Keychain fallback (`cfg(target_os = "macos")` for
//!   the `security(1)` shellouts; parser fns compiled cross-platform for
//!   testability).
//! - `http` — the `ureq` call, proxy resolution (`HTTPS_PROXY`/`NO_PROXY`
//!   with `https://`→`http://` and bare-`host:port` normalization for
//!   ureq 2.x), and `Retry-After` parsing.
//! - `cache` — on-disk JSON cache + per-outcome TTL enum.
//! - `lock` — cross-process file lock + `blocked_until_ms` marker.

mod cache;
mod credentials;
mod http;
mod keychain;
pub mod lock;

use std::time::{SystemTime, UNIX_EPOCH};

use glassline_core::render_context::{RenderUsageData, UsageError};

use self::{
    cache::{
        RATE_LIMIT_TTL_SECS, read_cache, read_cache_data_ignoring_ttl, ttl_for, usage_cache_path,
        write_cache,
    },
    credentials::resolve_access_token,
    http::{FetchError, fetch_from_api},
    lock::{LOCK_MAX_AGE_MS, LockReason, UsageLock, synthesize_backoff_data, usage_lock_path},
};

/// Public entry point used by `main.rs`.
///
/// Returns `None` only if we couldn't produce any [`RenderUsageData`] — the
/// error path already surfaces its own `UsageError` on the returned struct,
/// so widgets can render `[Timeout]` / `[Rate limited]` / etc.
pub fn fetch_or_cached() -> Option<RenderUsageData> {
    let now_ms = current_time_ms();

    let token = resolve_access_token().ok();
    let token_hash = token.as_deref().map(fingerprint_token);

    let cache_path = usage_cache_path()?;
    if let Some(cached) = read_cache(&cache_path)
        && cached.token_hash == token_hash
    {
        let ttl = ttl_for(cached.data.error);
        if now_ms.saturating_sub(cached.cached_at_ms) < ttl * 1000 {
            return Some(cached.data);
        }
    }

    let Some(token) = token else {
        let data = RenderUsageData {
            error: Some(UsageError::NoCredentials),
            ..Default::default()
        };
        write_cache(&cache_path, &data, token_hash, now_ms);
        return Some(data);
    };

    // Cache is stale (or absent) and we have a token → we may need to
    // hit the network. Coordinate with peers via the lock file.
    let lock_path = usage_lock_path()?;
    let Some(mut lock) = UsageLock::try_acquire(&lock_path) else {
        // D3: another glassline is fetching. Return whatever's cached,
        // fresh or stale. Never block the render pipeline.
        return read_cache_data_ignoring_ttl(&cache_path);
    };

    if let Some(body) = lock.read_body()
        && body.blocked_until_ms > now_ms
    {
        // A peer stamped a backoff. Prefer cached data if present so
        // widgets show the last-known values alongside the error; fall
        // back to a synthesized skeleton if we've never cached.
        let synthesized = synthesize_backoff_data(body.reason);
        return Some(read_cache_data_ignoring_ttl(&cache_path).unwrap_or(synthesized));
    }

    // Pre-fetch marker: if we crash mid-request, peers within 30s treat
    // us as timed-out and skip their own fetch. Matches TS LOCK_MAX_AGE.
    lock.write(now_ms + LOCK_MAX_AGE_MS, LockReason::Timeout);

    let mut retry_after_override: Option<u64> = None;
    let fetched = match fetch_from_api(&token) {
        Ok(body) => match parse_usage_response(&body) {
            Some(data) => data,
            None => RenderUsageData {
                error: Some(UsageError::ParseError),
                ..Default::default()
            },
        },
        Err(FetchError::RateLimited(retry_after)) => {
            retry_after_override = retry_after;
            RenderUsageData {
                error: Some(UsageError::RateLimited),
                ..Default::default()
            }
        }
        Err(FetchError::Timeout) => RenderUsageData {
            error: Some(UsageError::Timeout),
            ..Default::default()
        },
        Err(FetchError::Other) => RenderUsageData {
            error: Some(UsageError::ApiError),
            ..Default::default()
        },
    };

    // If Anthropic gave us a Retry-After hint on 429, honor it by
    // stamping the cache time backwards so the effective TTL matches.
    // e.g. Retry-After: 600 + RATE_LIMIT_TTL_SECS 300 -> stamp
    //      cache 300s in the past so read-back computes an 300s-in-
    //      the-future expiry (300 + 300 = 600s effective backoff).
    let stamp_at = if let Some(retry_after) = retry_after_override {
        let extra_ms = retry_after.saturating_sub(RATE_LIMIT_TTL_SECS) * 1000;
        now_ms.saturating_sub(extra_ms)
    } else {
        now_ms
    };
    write_cache(&cache_path, &fetched, token_hash, stamp_at);

    // Post-fetch lock update — peers use this to decide whether to fetch
    // themselves in the next 30s / rate-limit window.
    match fetched.error {
        Some(UsageError::RateLimited) => {
            let backoff_ms = retry_after_override
                .map(|s| s * 1000)
                .unwrap_or(RATE_LIMIT_TTL_SECS * 1000);
            lock.write(now_ms + backoff_ms, LockReason::RateLimited);
        }
        Some(UsageError::Timeout) => {
            lock.write(now_ms + LOCK_MAX_AGE_MS, LockReason::Timeout);
        }
        _ => {
            // Success (or non-network error): clear the marker so peers
            // are free to fetch after cache TTL expires. Past-timestamp
            // avoids the delete-vs-open race a `remove_file` would open.
            lock.write(0, LockReason::Timeout);
        }
    }

    Some(fetched)
}

// ---------- parse ----------

/// Public for reuse in unit tests.
#[must_use]
pub fn parse_usage_response(raw: &str) -> Option<RenderUsageData> {
    let root: serde_json::Value = serde_json::from_str(raw).ok()?;
    let five_hour = root.get("five_hour");
    let seven_day = root.get("seven_day");
    let seven_day_sonnet = root.get("seven_day_sonnet");
    let seven_day_opus = root.get("seven_day_opus");
    let extra_usage = root.get("extra_usage");
    let limits = root.get("limits").and_then(|v| v.as_array());

    let session_limit =
        limits.and_then(|arr| find_limit(arr, "weekly_all").or_else(|| find_limit(arr, "session")));
    let session_kind = limits.and_then(|arr| find_limit(arr, "session"));
    let weekly_kind = limits.and_then(|arr| find_limit(arr, "weekly_all"));
    let sonnet_scoped = limits.and_then(|arr| find_scoped_limit(arr, "sonnet"));
    let opus_scoped = limits.and_then(|arr| find_scoped_limit(arr, "opus"));

    let _ = session_limit;

    Some(RenderUsageData {
        session_usage: bucket_utilization(five_hour).or_else(|| limit_percent(session_kind)),
        session_reset_at: bucket_resets_at(five_hour).or_else(|| limit_resets_at(session_kind)),
        weekly_usage: bucket_utilization(seven_day).or_else(|| limit_percent(weekly_kind)),
        weekly_reset_at: bucket_resets_at(seven_day).or_else(|| limit_resets_at(weekly_kind)),
        weekly_sonnet_usage: limit_percent(sonnet_scoped)
            .or_else(|| bucket_utilization(seven_day_sonnet)),
        weekly_sonnet_reset_at: limit_resets_at(sonnet_scoped)
            .or_else(|| bucket_resets_at(seven_day_sonnet)),
        weekly_opus_usage: limit_percent(opus_scoped)
            .or_else(|| bucket_utilization(seven_day_opus)),
        weekly_opus_reset_at: limit_resets_at(opus_scoped)
            .or_else(|| bucket_resets_at(seven_day_opus)),
        fable_usage: None,
        fable_reset_at: None,
        extra_usage_enabled: extra_usage
            .and_then(|v| v.get("is_enabled"))
            .and_then(|v| v.as_bool()),
        extra_usage_limit: extra_usage
            .and_then(|v| v.get("monthly_limit"))
            .and_then(|v| v.as_f64()),
        extra_usage_used: extra_usage
            .and_then(|v| v.get("used_credits"))
            .and_then(|v| v.as_f64()),
        extra_usage_utilization: extra_usage
            .and_then(|v| v.get("utilization"))
            .and_then(|v| v.as_f64()),
        extra_usage_currency: extra_usage
            .and_then(|v| v.get("currency"))
            .and_then(|v| v.as_str().map(String::from)),
        error: None,
    })
}

fn bucket_utilization(bucket: Option<&serde_json::Value>) -> Option<f64> {
    let b = bucket?;
    if b.is_null() {
        return Some(0.0);
    }
    b.get("utilization").and_then(|v| v.as_f64())
}

fn bucket_resets_at(bucket: Option<&serde_json::Value>) -> Option<String> {
    bucket?.get("resets_at")?.as_str().map(String::from)
}

fn find_limit<'a>(limits: &'a [serde_json::Value], kind: &str) -> Option<&'a serde_json::Value> {
    limits
        .iter()
        .find(|v| v.get("kind").and_then(|k| k.as_str()) == Some(kind))
}

fn find_scoped_limit<'a>(
    limits: &'a [serde_json::Value],
    model_needle: &str,
) -> Option<&'a serde_json::Value> {
    let needle = model_needle.to_lowercase();
    limits.iter().find(|v| {
        if v.get("kind").and_then(|k| k.as_str()) != Some("weekly_scoped") {
            return false;
        }
        let name = v
            .get("scope")
            .and_then(|s| s.get("model"))
            .and_then(|m| m.get("display_name"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_lowercase();
        name.contains(&needle)
    })
}

fn limit_percent(limit: Option<&serde_json::Value>) -> Option<f64> {
    let l = limit?;
    if is_placeholder(l) {
        return None;
    }
    l.get("percent").and_then(|v| v.as_f64())
}

fn limit_resets_at(limit: Option<&serde_json::Value>) -> Option<String> {
    let l = limit?;
    if is_placeholder(l) {
        return None;
    }
    l.get("resets_at")
        .and_then(|v| v.as_str().map(String::from))
}

fn is_placeholder(limit: &serde_json::Value) -> bool {
    let pct = limit.get("percent").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let resets = limit.get("resets_at").is_some_and(|v| !v.is_null());
    pct == 0.0 && !resets
}

// ---------- misc ----------

fn fingerprint_token(token: &str) -> String {
    // Not cryptographic — just a stable, cheap short identifier that
    // detects login switches without needing sha2 as a dep. Length + first
    // 8 chars is enough to catch a token swap.
    format!(
        "{}:{}",
        token.len(),
        token.chars().take(8).collect::<String>()
    )
}

pub(crate) fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RESPONSE: &str = r#"{
        "five_hour": {"utilization": 45.0, "resets_at": "2026-08-18T19:39:59Z"},
        "seven_day": {"utilization": 55.0, "resets_at": "2026-08-20T04:59:59Z"},
        "seven_day_sonnet": {"utilization": 12.5, "resets_at": "2026-08-20T04:59:59Z"},
        "seven_day_opus": {"utilization": 8.0, "resets_at": "2026-08-20T04:59:59Z"},
        "extra_usage": {"is_enabled": false, "monthly_limit": null, "used_credits": 0, "utilization": 0, "currency": "USD"},
        "limits": [
            {"kind": "session", "percent": 45, "resets_at": "2026-08-18T19:39:59Z", "scope": null},
            {"kind": "weekly_all", "percent": 55, "resets_at": "2026-08-20T04:59:59Z", "scope": null},
            {"kind": "weekly_scoped", "percent": 12, "resets_at": "2026-08-20T04:59:59Z", "scope": {"model": {"display_name": "Sonnet 4.7"}}}
        ]
    }"#;

    #[test]
    fn parses_five_hour_and_seven_day() {
        let out = parse_usage_response(SAMPLE_RESPONSE).unwrap();
        assert_eq!(out.session_usage, Some(45.0));
        assert_eq!(out.weekly_usage, Some(55.0));
        // limits[] weekly_scoped (12) is authoritative over the legacy
        // seven_day_sonnet bucket (12.5) per TS parseUsageApiResponse.
        assert_eq!(out.weekly_sonnet_usage, Some(12.0));
        assert_eq!(out.weekly_opus_usage, Some(8.0));
    }

    #[test]
    fn parses_limits_fallback() {
        let raw = r#"{
            "limits": [
                {"kind": "session", "percent": 30, "resets_at": "2026-08-18T20:00:00Z", "scope": null},
                {"kind": "weekly_all", "percent": 42, "resets_at": "2026-08-20T00:00:00Z", "scope": null}
            ]
        }"#;
        let out = parse_usage_response(raw).unwrap();
        assert_eq!(out.session_usage, Some(30.0));
        assert_eq!(out.weekly_usage, Some(42.0));
    }

    #[test]
    fn fingerprint_stable() {
        let a = fingerprint_token("abcdefghijklmnop");
        let b = fingerprint_token("abcdefghijklmnop");
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_differs_on_swap() {
        let a = fingerprint_token("abcdefghijklmnop");
        let b = fingerprint_token("qrstuvwxyz012345");
        assert_ne!(a, b);
    }

    #[test]
    fn placeholder_limit_dropped() {
        let raw = r#"{
            "limits": [
                {"kind": "session", "percent": 0, "resets_at": null, "scope": null}
            ]
        }"#;
        let out = parse_usage_response(raw).unwrap();
        assert_eq!(out.session_usage, None);
    }
}
