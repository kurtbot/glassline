//! HTTP fetch layer for the Anthropic OAuth usage endpoint.
//!
//! Isolates the `ureq` call, error classification, `Retry-After`
//! parsing, and proxy resolution (`HTTPS_PROXY`/`NO_PROXY`) so the
//! top-level orchestrator in `mod.rs` stays readable. See

use std::time::Duration;

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const USAGE_HOST: &str = "api.anthropic.com";
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(super) enum FetchError {
    /// 429 with an optional `Retry-After` value (in seconds). None means
    /// Anthropic didn't send the header; caller falls back to the
    /// default rate-limit TTL from `cache`.
    RateLimited(Option<u64>),
    Timeout,
    Other,
}

pub(super) fn fetch_from_api(token: &str) -> Result<String, FetchError> {
    let agent = build_agent();
    let req = agent
        .get(USAGE_URL)
        .set("Authorization", &format!("Bearer {token}"))
        // TS ccstatusline sends this beta header; without it Anthropic
        // treats the client as non-OAuth and applies a stricter rate
        // limit (empirically the cause of ambient 429s in our port).
        // See utils/usage-fetch.ts:641.
        .set("anthropic-beta", "oauth-2025-04-20")
        .set(
            "User-Agent",
            concat!("glassline/", env!("CARGO_PKG_VERSION")),
        );
    match req.call() {
        Ok(resp) => resp.into_string().map_err(|_| FetchError::Other),
        Err(ureq::Error::Status(429, resp)) => {
            let retry_after =
                parse_retry_after(resp.header("Retry-After"), super::current_time_ms());
            Err(FetchError::RateLimited(retry_after))
        }
        Err(ureq::Error::Transport(t)) => {
            if matches!(t.kind(), ureq::ErrorKind::Io) {
                Err(FetchError::Timeout)
            } else {
                Err(FetchError::Other)
            }
        }
        Err(_) => Err(FetchError::Other),
    }
}

/// Build a `ureq::Agent`, honoring `HTTPS_PROXY` / `https_proxy` /
/// `NO_PROXY` env vars. Invalid proxy URLs are silently ignored so a
/// misconfigured proxy never bricks the render pipeline (G5).
fn build_agent() -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new().timeout(HTTP_TIMEOUT);
    if let Some(url) = resolve_proxy_url(USAGE_HOST)
        && let Ok(proxy) = ureq::Proxy::new(&url)
    {
        builder = builder.proxy(proxy);
    }
    builder.build()
}

/// Resolve the proxy URL for a target host, or `None` if none applies.
///
/// Reads `HTTPS_PROXY` (uppercase preferred) then `https_proxy`. Empty
/// string after trim is treated as unset. `NO_PROXY` matching disables
/// the proxy. Result URL is normalized so `ureq::Proxy::new` accepts
/// it: `https://` → `http://`, bare `host:port` → `http://host:port`.
fn resolve_proxy_url(host: &str) -> Option<String> {
    resolve_proxy_url_from(host, &|k| std::env::var(k).ok())
}

fn resolve_proxy_url_from<F>(host: &str, env: &F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    let raw = env("HTTPS_PROXY").or_else(|| env("https_proxy"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_no_proxy_from(host, env) {
        return None;
    }
    Some(normalize_proxy_scheme(trimmed))
}

fn is_no_proxy_from<F>(host: &str, env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    let list = env("NO_PROXY").or_else(|| env("no_proxy"));
    let Some(list) = list else {
        return false;
    };
    let host_lower = host.to_lowercase();
    for entry in list.split(',') {
        let e = entry.trim();
        if e.is_empty() {
            continue;
        }
        if e == "*" {
            return true;
        }
        if host_lower.eq_ignore_ascii_case(e) {
            return true;
        }
        if let Some(suffix) = e.strip_prefix('.') {
            let suffix = suffix.to_lowercase();
            // `.suffix` matches the apex (`host == suffix`) AND any
            // proper subdomain (`host` ends with `.suffix`). Requiring
            // a dot boundary rejects false positives like
            // `mimicanthropic.com` matching `.anthropic.com`.
            if host_lower == suffix {
                return true;
            }
            if let Some(prefix) = host_lower.strip_suffix(&suffix)
                && prefix.ends_with('.')
            {
                return true;
            }
        }
    }
    false
}

/// Normalize a proxy URL so `ureq 2.x::Proxy::new` accepts it. ureq
/// 2.x rejects the `https://` scheme (only `http://` / `socks*://`);
/// HTTPS_PROXY values conventionally use `http://` anyway because the
/// client-to-proxy handshake is plain HTTP CONNECT.
fn normalize_proxy_scheme(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        return format!("http://{rest}");
    }
    if url.contains("://") {
        return url.to_string();
    }
    format!("http://{url}")
}

/// Parse a `Retry-After` value. Accepts both integer-seconds
/// (`"120"`) and HTTP-date forms (`"Wed, 21 Oct 2015 07:28:00 GMT"`)
/// per RFC 7231. Returns `None` on empty / unparseable / past-date.
pub(super) fn parse_retry_after(header: Option<&str>, now_ms: u64) -> Option<u64> {
    let raw = header?.trim();
    if raw.is_empty() {
        return None;
    }
    if raw.chars().all(|c| c.is_ascii_digit()) {
        return raw.parse().ok().filter(|v: &u64| *v > 0);
    }
    let parsed =
        time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc2822).ok()?;
    let retry_at_ms = parsed.unix_timestamp_nanos() / 1_000_000;
    let delta = retry_at_ms - now_ms as i128;
    if delta <= 0 {
        None
    } else {
        Some((delta / 1000) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + 'static {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    // ---------- retry_after ----------

    #[test]
    fn retry_after_integer_seconds() {
        assert_eq!(parse_retry_after(Some("120"), 0), Some(120));
    }

    #[test]
    fn retry_after_none_on_empty_or_junk() {
        assert_eq!(parse_retry_after(None, 0), None);
        assert_eq!(parse_retry_after(Some(""), 0), None);
        assert_eq!(parse_retry_after(Some("   "), 0), None);
        assert_eq!(parse_retry_after(Some("not-a-date"), 0), None);
    }

    #[test]
    fn retry_after_http_date_form() {
        assert_eq!(
            parse_retry_after(Some("Thu, 01 Jan 1970 00:01:00 GMT"), 0),
            Some(60)
        );
    }

    #[test]
    fn retry_after_past_date_returns_none() {
        assert_eq!(
            parse_retry_after(Some("Thu, 01 Jan 1970 00:00:00 GMT"), 60_000),
            None
        );
    }

    // ---------- normalize_proxy_scheme ----------

    #[test]
    fn normalize_https_becomes_http() {
        assert_eq!(
            normalize_proxy_scheme("https://proxy.example.com:8080"),
            "http://proxy.example.com:8080"
        );
    }

    #[test]
    fn normalize_http_passthrough() {
        assert_eq!(
            normalize_proxy_scheme("http://proxy.example.com:8080"),
            "http://proxy.example.com:8080"
        );
    }

    #[test]
    fn normalize_socks5_passthrough() {
        assert_eq!(
            normalize_proxy_scheme("socks5://user:pass@proxy:1080"),
            "socks5://user:pass@proxy:1080"
        );
    }

    #[test]
    fn normalize_bare_hostport_gets_http_scheme() {
        assert_eq!(
            normalize_proxy_scheme("proxy.example.com:8080"),
            "http://proxy.example.com:8080"
        );
    }

    // ---------- resolve_proxy_url ----------

    #[test]
    fn resolve_uses_uppercase_over_lowercase() {
        let env = env_of(&[
            ("HTTPS_PROXY", "http://upper:8080"),
            ("https_proxy", "http://lower:8080"),
        ]);
        assert_eq!(
            resolve_proxy_url_from("api.anthropic.com", &env),
            Some("http://upper:8080".to_string())
        );
    }

    #[test]
    fn resolve_falls_back_to_lowercase() {
        let env = env_of(&[("https_proxy", "http://lower:8080")]);
        assert_eq!(
            resolve_proxy_url_from("api.anthropic.com", &env),
            Some("http://lower:8080".to_string())
        );
    }

    #[test]
    fn resolve_empty_string_returns_none() {
        let env = env_of(&[("HTTPS_PROXY", "")]);
        assert_eq!(resolve_proxy_url_from("api.anthropic.com", &env), None);
    }

    #[test]
    fn resolve_whitespace_only_returns_none() {
        let env = env_of(&[("HTTPS_PROXY", "   \t  ")]);
        assert_eq!(resolve_proxy_url_from("api.anthropic.com", &env), None);
    }

    #[test]
    fn resolve_absent_returns_none() {
        let env = env_of(&[]);
        assert_eq!(resolve_proxy_url_from("api.anthropic.com", &env), None);
    }

    #[test]
    fn resolve_normalizes_https_input() {
        let env = env_of(&[("HTTPS_PROXY", "https://proxy:8080")]);
        assert_eq!(
            resolve_proxy_url_from("api.anthropic.com", &env),
            Some("http://proxy:8080".to_string())
        );
    }

    #[test]
    fn resolve_no_proxy_match_disables() {
        let env = env_of(&[
            ("HTTPS_PROXY", "http://proxy:8080"),
            ("NO_PROXY", "api.anthropic.com"),
        ]);
        assert_eq!(resolve_proxy_url_from("api.anthropic.com", &env), None);
    }

    // ---------- is_no_proxy ----------

    #[test]
    fn no_proxy_exact_match() {
        let env = env_of(&[("NO_PROXY", "api.anthropic.com")]);
        assert!(is_no_proxy_from("api.anthropic.com", &env));
        assert!(!is_no_proxy_from("other.example.com", &env));
    }

    #[test]
    fn no_proxy_case_insensitive() {
        let env = env_of(&[("NO_PROXY", "API.Anthropic.COM")]);
        assert!(is_no_proxy_from("api.anthropic.com", &env));
    }

    #[test]
    fn no_proxy_suffix_with_leading_dot() {
        let env = env_of(&[("NO_PROXY", ".anthropic.com")]);
        // Subdomains match.
        assert!(is_no_proxy_from("api.anthropic.com", &env));
        assert!(is_no_proxy_from("cdn.anthropic.com", &env));
        // Apex matches (matches cURL / requests / reqwest convention).
        assert!(is_no_proxy_from("anthropic.com", &env));
        // Look-alike hostnames must NOT match — dot boundary required.
        assert!(!is_no_proxy_from("mimicanthropic.com", &env));
    }

    #[test]
    fn no_proxy_wildcard_star() {
        let env = env_of(&[("NO_PROXY", "*")]);
        assert!(is_no_proxy_from("api.anthropic.com", &env));
        assert!(is_no_proxy_from("anything", &env));
    }

    #[test]
    fn no_proxy_multiple_comma_separated() {
        let env = env_of(&[("NO_PROXY", "example.com, .anthropic.com , foo.bar")]);
        assert!(is_no_proxy_from("api.anthropic.com", &env));
        assert!(is_no_proxy_from("example.com", &env));
        assert!(is_no_proxy_from("foo.bar", &env));
        assert!(!is_no_proxy_from("other.com", &env));
    }

    #[test]
    fn no_proxy_empty_entries_tolerated() {
        let env = env_of(&[("NO_PROXY", ",,api.anthropic.com,,")]);
        assert!(is_no_proxy_from("api.anthropic.com", &env));
    }

    #[test]
    fn no_proxy_lowercase_env_var() {
        let env = env_of(&[("no_proxy", "api.anthropic.com")]);
        assert!(is_no_proxy_from("api.anthropic.com", &env));
    }

    #[test]
    fn no_proxy_absent() {
        let env = env_of(&[]);
        assert!(!is_no_proxy_from("api.anthropic.com", &env));
    }
}
