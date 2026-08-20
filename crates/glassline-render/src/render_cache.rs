//! Micro-cache in front of the render pipeline.
//!
//! Design ref: [[render_cache_design_v1.0]].
//!
//! Claude Code invokes glassline on every event (debounced to ~300ms) plus
//! on a user-configured `refreshInterval` (seconds, minimum 1). Doing the
//! full transcript scan + git shell-outs + usage probe on every one of
//! those invocations wastes CPU when stdin and settings haven't changed.
//!
//! This module implements a best-effort on-disk cache keyed on
//! `{hash(stdin), settings.json mtime, floor(now_ms / TTL_MS), version}`.
//! Time-bucket quantization keeps animation working (frames still advance
//! across bucket boundaries) while identical invocations within a bucket
//! replay the previous ANSI blob.
//!
//! Fail-open everywhere. Corrupt file, missing dir, contention, mismatched
//! version — all short-circuit to a fresh render.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

/// Default TTL when `GLASSLINE_RENDER_TTL_MS` is unset. Chosen at 150ms —
/// below Claude Code's 300ms event debounce so burst-coalesced invocations
/// fall in one bucket, but short enough to keep animation visible.
pub const DEFAULT_TTL_MS: u64 = 150;

/// Upper clamp for the env override. Values above this reduce animation
/// FPS below the 30s usage-cache TTL layer — nothing gained by going higher.
const MAX_TTL_MS: u64 = 5_000;

/// Composite cache key. Two invocations replay each other iff all four
/// fields match.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheKey {
    /// FNV-1a 64-bit hash of stdin bytes, as lowercase hex.
    ///
    /// FNV-1a picked over SHA-256 to avoid adding the `sha2` crate. Cache
    /// is per-user single-machine; a collision would produce one wrong
    /// frame for at most one TTL window before the next invocation
    /// re-renders. Non-crypto is fine.
    pub stdin_hash: String,
    /// Nanoseconds-since-epoch of `settings.json`'s mtime, `0` when the
    /// file can't be stat'd (fall-open — the cache still works, just with
    /// less cross-save locality).
    pub settings_mtime_ns: u128,
    /// `floor(now_ms / TTL_MS)`. Same across a TTL window; changes across
    /// boundaries so animation phase can advance.
    pub time_bucket: u64,
    /// Compile-time glassline version. Invalidates cache entries written
    /// by an older binary without needing a schema-version field. Owned
    /// String so Deserialize doesn't fight the `'static` lifetime — the
    /// storage cost (~10 bytes) is negligible.
    pub version: String,
}

/// Wire format of the cache file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    key: CacheKey,
    ansi_output: String,
    /// Milliseconds-since-epoch when written. Used as a sanity guard for
    /// clock-rewound + very-stale scenarios (>= 2×TTL treated as expired).
    written_at_ms: u64,
}

/// FNV-1a 64-bit hash, lowercase hex (16 chars). Public for tests + so the
/// import subcommand can compose the same key when it becomes a
/// render-cache consumer (v1.1).
#[must_use]
pub fn hash_stdin(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    format!("{h:016x}")
}

/// Resolve `~/.cache/glassline/render.cache`, or the platform equivalent.
#[must_use]
pub fn cache_path() -> Option<PathBuf> {
    if cfg!(windows)
        && let Some(local) = std::env::var_os("LOCALAPPDATA")
    {
        return Some(PathBuf::from(local).join("glassline").join("render.cache"));
    }
    let home = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache"))
        })
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(home.join("glassline").join("render.cache"))
}

/// TTL from env or default. Values of 0 disable the cache entirely.
#[must_use]
pub fn ttl_ms() -> u64 {
    let Some(raw) = std::env::var_os("GLASSLINE_RENDER_TTL_MS") else {
        return DEFAULT_TTL_MS;
    };
    let Some(s) = raw.to_str() else {
        return DEFAULT_TTL_MS;
    };
    let Ok(n) = s.parse::<u64>() else {
        return DEFAULT_TTL_MS;
    };
    n.min(MAX_TTL_MS)
}

/// True when `GLASSLINE_CACHE_STATS=1` — opt-in hit/miss telemetry into
/// `debug.log` (see [`record_stat`]).
#[must_use]
pub fn stats_enabled() -> bool {
    std::env::var_os("GLASSLINE_CACHE_STATS")
        .and_then(|v| v.into_string().ok())
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

/// Build a cache key. `settings_path` mtime is best-effort — a missing
/// file becomes `settings_mtime_ns = 0`, still a valid key.
#[must_use]
pub fn build_key(
    stdin_bytes: &[u8],
    settings_path: Option<&std::path::Path>,
    now_ms: u64,
    ttl: u64,
) -> CacheKey {
    let settings_mtime_ns = settings_path
        .and_then(|p| fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let time_bucket = now_ms.checked_div(ttl).unwrap_or(now_ms);
    CacheKey {
        stdin_hash: hash_stdin(stdin_bytes),
        settings_mtime_ns,
        time_bucket,
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

/// Try to read the cache entry and return its ANSI output on match.
///
/// Returns `None` on any of: TTL disabled, no cache path, missing file,
/// I/O error, JSON parse error, key mismatch, clock rewound, or stale
/// entry (>= 2×TTL old). Never panics.
///
/// **No lock.** Writers use tmp+rename, so a concurrent write either
/// completes atomically before or after the read. A partial-tmp is never
/// visible under the target name. Worst case is reading a stale-but-valid
/// entry across the tmp+rename boundary — that's already handled by the
/// key comparison and the `written_at_ms` staleness guard.
#[must_use]
pub fn try_read(key: &CacheKey, now_ms: u64) -> Option<String> {
    let ttl = ttl_ms();
    if ttl == 0 {
        return None;
    }
    let path = cache_path()?;
    let raw = fs::read_to_string(&path).ok()?;
    let entry: CacheEntry = serde_json::from_str(&raw).ok()?;
    if entry.key != *key {
        return None;
    }
    if now_ms < entry.written_at_ms {
        return None; // clock rewound
    }
    if now_ms.saturating_sub(entry.written_at_ms) > ttl.saturating_mul(2) {
        return None; // way stale
    }
    Some(entry.ansi_output)
}

/// Write the entry to disk atomically (tmp + rename). Silent on failure.
pub fn write(key: &CacheKey, ansi_output: &str, now_ms: u64) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(dir) = path.parent()
        && fs::create_dir_all(dir).is_err()
    {
        return;
    }
    let entry = CacheEntry {
        key: key.clone(),
        ansi_output: ansi_output.to_string(),
        written_at_ms: now_ms,
    };
    let Ok(bytes) = serde_json::to_vec(&entry) else {
        return;
    };
    let tmp = path.with_extension("cache.tmp");
    if fs::write(&tmp, &bytes).is_err() {
        return;
    }
    let _ = fs::rename(&tmp, &path);
}

/// Opt-in telemetry helper — appends one line to `~/.cache/glassline/debug.log`
/// when `GLASSLINE_CACHE_STATS=1`. Silent no-op otherwise.
pub fn record_stat(hit: bool, now_ms: u64) {
    if !stats_enabled() {
        return;
    }
    let Some(dir) = cache_path().and_then(|p| p.parent().map(std::path::Path::to_path_buf)) else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    let log_path = dir.join("debug.log");
    let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) else {
        return;
    };
    let _ = writeln!(f, "[{now_ms}] cache: {}", if hit { "hit" } else { "miss" });
    let _ = f.flush();
}

#[cfg(test)]
#[allow(unsafe_code)]
// std::env::set_var / remove_var are `unsafe` in Rust 2024 — required
// here because these tests exercise env-driven behaviour (`ttl_ms()`,
// `stats_enabled()`, `cache_path()` resolution). All env mutations
// serialise on TEST_ENV_LOCK below.
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

    /// Serialises env-var mutation across tests in this module. Cargo runs
    /// unit tests in parallel by default; every test in this file touches
    /// `GLASSLINE_RENDER_TTL_MS`, `GLASSLINE_CACHE_STATS`, `LOCALAPPDATA`,
    /// or `XDG_CACHE_HOME`, so they must serialise or race each other.
    static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn dummy_key(stdin: &[u8], mtime: u128, bucket: u64) -> CacheKey {
        CacheKey {
            stdin_hash: hash_stdin(stdin),
            settings_mtime_ns: mtime,
            time_bucket: bucket,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    fn set_cache_path_to(dir: &Path) {
        // The public `cache_path()` reads platform env vars — for tests
        // point LOCALAPPDATA (Windows) and XDG_CACHE_HOME (Unix) at our
        // temp dir + a synthetic `glassline` subpath so the resolver
        // constructs `<dir>/glassline/render.cache`.
        #[cfg(windows)]
        unsafe {
            std::env::set_var("LOCALAPPDATA", dir);
        }
        #[cfg(not(windows))]
        unsafe {
            std::env::set_var("XDG_CACHE_HOME", dir);
        }
    }

    fn temp_cache_dir(name: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("glassline-cache-{name}-"))
            .tempdir()
            .expect("tempdir")
    }

    #[test]
    fn fnv1a_is_deterministic() {
        assert_eq!(hash_stdin(b"hello"), hash_stdin(b"hello"));
        assert_ne!(hash_stdin(b"hello"), hash_stdin(b"world"));
    }

    #[test]
    fn fnv1a_output_is_16_hex_chars() {
        let h = hash_stdin(b"anything");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn ttl_ms_default_when_unset() {
        // Serialise access to the env var so parallel tests don't race.
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("GLASSLINE_RENDER_TTL_MS");
        }
        assert_eq!(ttl_ms(), DEFAULT_TTL_MS);
    }

    #[test]
    fn ttl_ms_env_override() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("GLASSLINE_RENDER_TTL_MS", "300");
        }
        assert_eq!(ttl_ms(), 300);
        unsafe {
            std::env::set_var("GLASSLINE_RENDER_TTL_MS", "9999999");
        }
        assert_eq!(ttl_ms(), MAX_TTL_MS);
        unsafe {
            std::env::set_var("GLASSLINE_RENDER_TTL_MS", "0");
        }
        assert_eq!(ttl_ms(), 0);
        unsafe {
            std::env::remove_var("GLASSLINE_RENDER_TTL_MS");
        }
    }

    #[test]
    fn ttl_zero_disables_cache() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("ttl-zero");
        set_cache_path_to(dir.path());
        unsafe {
            std::env::set_var("GLASSLINE_RENDER_TTL_MS", "0");
        }
        let key = dummy_key(b"x", 0, 0);
        write(&key, "cached-output", 1_000);
        assert!(
            try_read(&key, 1_000).is_none(),
            "TTL=0 must always miss"
        );
        unsafe {
            std::env::remove_var("GLASSLINE_RENDER_TTL_MS");
        }
    }

    #[test]
    fn write_then_read_round_trips() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("roundtrip");
        set_cache_path_to(dir.path());
        unsafe {
            std::env::remove_var("GLASSLINE_RENDER_TTL_MS");
        }
        let key = dummy_key(b"payload", 12345, 100);
        write(&key, "hello ansi", 1_000);
        let read = try_read(&key, 1_050).expect("cache hit");
        assert_eq!(read, "hello ansi");
    }

    #[test]
    fn miss_on_stdin_change() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("miss-stdin");
        set_cache_path_to(dir.path());
        let write_key = dummy_key(b"payload-A", 12345, 100);
        write(&write_key, "output-A", 1_000);
        let read_key = dummy_key(b"payload-B", 12345, 100);
        assert!(try_read(&read_key, 1_050).is_none());
    }

    #[test]
    fn miss_on_settings_mtime_change() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("miss-mtime");
        set_cache_path_to(dir.path());
        let write_key = dummy_key(b"payload", 12345, 100);
        write(&write_key, "output", 1_000);
        let read_key = dummy_key(b"payload", 99999, 100);
        assert!(try_read(&read_key, 1_050).is_none());
    }

    #[test]
    fn miss_on_time_bucket_change() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("miss-bucket");
        set_cache_path_to(dir.path());
        let write_key = dummy_key(b"payload", 12345, 100);
        write(&write_key, "output", 1_000);
        let read_key = dummy_key(b"payload", 12345, 101);
        assert!(try_read(&read_key, 1_050).is_none());
    }

    #[test]
    fn miss_on_clock_rewound() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("miss-rewound");
        set_cache_path_to(dir.path());
        let key = dummy_key(b"payload", 12345, 100);
        write(&key, "output", 5_000);
        // Read at earlier now_ms — entry.written_at_ms > now_ms.
        assert!(try_read(&key, 4_000).is_none());
    }

    #[test]
    fn miss_on_way_stale_entry() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("miss-stale");
        set_cache_path_to(dir.path());
        unsafe {
            std::env::set_var("GLASSLINE_RENDER_TTL_MS", "100");
        }
        let key = dummy_key(b"payload", 12345, 100);
        write(&key, "output", 1_000);
        // 2×TTL later = 1200; the read at 1500 should miss.
        assert!(try_read(&key, 1_500).is_none());
        unsafe {
            std::env::remove_var("GLASSLINE_RENDER_TTL_MS");
        }
    }

    #[test]
    fn corrupt_cache_body_falls_through() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("corrupt");
        set_cache_path_to(dir.path());
        let path = cache_path().expect("cache_path");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not-json {{{").unwrap();
        let key = dummy_key(b"payload", 12345, 100);
        assert!(try_read(&key, 1_000).is_none());
    }

    #[test]
    fn missing_dir_write_is_silent() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        // Point cache at a path whose grandparent doesn't exist but parent
        // can be created — write() calls create_dir_all so this must succeed.
        let dir = temp_cache_dir("nested");
        let nested = dir.path().join("a").join("b").join("c");
        set_cache_path_to(&nested);
        let key = dummy_key(b"payload", 12345, 100);
        write(&key, "output", 1_000);
        assert!(
            try_read(&key, 1_050).is_some(),
            "write should have created the nested dir"
        );
    }

    #[test]
    fn stats_off_by_default() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::remove_var("GLASSLINE_CACHE_STATS");
        }
        assert!(!stats_enabled());
    }

    #[test]
    fn stats_on_when_env_1() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("GLASSLINE_CACHE_STATS", "1");
        }
        assert!(stats_enabled());
        unsafe {
            std::env::remove_var("GLASSLINE_CACHE_STATS");
        }
    }

    #[test]
    fn build_key_sets_all_four_fields() {
        let key = build_key(b"payload", None, 5_000, 150);
        assert_eq!(key.stdin_hash, hash_stdin(b"payload"));
        assert_eq!(key.settings_mtime_ns, 0);
        assert_eq!(key.time_bucket, 5_000 / 150);
        assert_eq!(key.version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn cache_path_uses_env_override() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        let dir = temp_cache_dir("path");
        set_cache_path_to(dir.path());
        let path = cache_path().expect("cache_path");
        assert!(path.starts_with(dir.path()));
        assert!(path.ends_with(Path::new("glassline").join("render.cache")));
    }
}
