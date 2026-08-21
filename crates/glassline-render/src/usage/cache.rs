//! On-disk cache for the Anthropic OAuth usage endpoint.
//!
//! Path lives at `%LOCALAPPDATA%\glassline\usage.json` (Windows) or
//! `$XDG_CACHE_HOME/glassline/usage.json` (unix). TTLs are outcome-scoped:
//! success 300s, generic error 30s, rate-limit 300s. Matches TS
//! `CACHE_MAX_AGE` / `DEFAULT_RATE_LIMIT_BACKOFF` semantics — see
//! F1 tracks the 300→180s alignment.

use std::{
    fs,
    path::{Path, PathBuf},
};

use glassline_core::render_context::{RenderUsageData, UsageError};
use serde::{Deserialize, Serialize};

/// Cache successful usage responses for 5 minutes. Usage percentages
/// change on the order of turns, not seconds, so a 5-minute refresh is
/// plenty and keeps our load on Anthropic's usage endpoint minimal
/// (at most 12 calls/hour). TS ccstatusline uses 180s (F1 backlog).
pub(super) const SUCCESS_TTL_SECS: u64 = 300;
/// Default TTL for a cached error entry. Kept short so most transient
/// failures (network flap, timeout) heal within a couple of refreshes.
pub(super) const ERROR_TTL_SECS: u64 = 30;
/// TTL for a 429 specifically — Anthropic's rate-limit window is much
/// longer than a transient network error's, and retrying every 30s while
/// still rate-limited just extends the ban. Matches TS ccstatusline's
/// `DEFAULT_RATE_LIMIT_BACKOFF = 300` in `utils/usage-fetch.ts`.
pub(super) const RATE_LIMIT_TTL_SECS: u64 = 300;

pub(super) fn usage_cache_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|h| PathBuf::from(h).join("AppData").join("Local"))
            })?
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?
    };
    Some(base.join("glassline").join("usage.json"))
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct UsageCacheFile {
    #[serde(default)]
    pub token_hash: Option<String>,
    pub cached_at_ms: u64,
    pub data: RenderUsageData,
}

pub(super) fn read_cache(path: &Path) -> Option<UsageCacheFile> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Return whatever data is in the cache, ignoring TTL. Used on lock
/// contention (design D3) — we return stale cache rather than block.
pub(super) fn read_cache_data_ignoring_ttl(path: &Path) -> Option<RenderUsageData> {
    read_cache(path).map(|c| c.data)
}

pub(super) fn write_cache(
    path: &Path,
    data: &RenderUsageData,
    token_hash: Option<String>,
    now_ms: u64,
) {
    let Some(parent) = path.parent() else { return };
    let _ = fs::create_dir_all(parent);
    let cache = UsageCacheFile {
        token_hash,
        cached_at_ms: now_ms,
        data: data.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&cache) else {
        return;
    };
    let _ = fs::write(path, bytes);
}

/// Pick the cache-TTL for a cached entry based on which error (if any)
/// it recorded. Rate-limit backs off long enough for Anthropic's window
/// to reset; other errors heal fast; success uses [`SUCCESS_TTL_SECS`].
pub(super) fn ttl_for(error: Option<UsageError>) -> u64 {
    match error {
        None => SUCCESS_TTL_SECS,
        Some(UsageError::RateLimited) => RATE_LIMIT_TTL_SECS,
        Some(_) => ERROR_TTL_SECS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_uses_long_ttl() {
        assert_eq!(ttl_for(Some(UsageError::RateLimited)), RATE_LIMIT_TTL_SECS);
        assert!(ttl_for(Some(UsageError::RateLimited)) > ttl_for(Some(UsageError::Timeout)));
    }

    #[test]
    fn success_uses_success_ttl() {
        assert_eq!(ttl_for(None), SUCCESS_TTL_SECS);
    }
}
