//! Cross-process lock file for the Anthropic OAuth usage endpoint.
//!
//! Serializes concurrent glassline invocations on the same machine so
//! at most one process hits `/api/oauth/usage` per 30 seconds and honors
//! Retry-After / rate-limit windows collectively. Non-blocking: on
//! contention we return stale cache instead of waiting.
//!

use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use fs2::FileExt;
use glassline_core::render_context::{RenderUsageData, UsageError};
use serde::{Deserialize, Serialize};

/// Pre-fetch marker TTL — a lock body written just before the HTTP
/// request expires 30s later. Matches TS ccstatusline's `LOCK_MAX_AGE`.
pub(super) const LOCK_MAX_AGE_MS: u64 = 30_000;

/// If a lock file's mtime is older than this, treat the body as absent.
/// Guards against a wedged process leaving a far-future `blocked_until_ms`.
const STALE_LOCK_MAX_AGE_SECS: u64 = 24 * 60 * 60;

/// JSON body of the lock file. Matches the shape of TS ccstatusline's
/// `UsageLockSchema` in `utils/usage-fetch.ts` conceptually (snake_case
/// used here since the file is glassline-only — not shared with TS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockBody {
    pub blocked_until_ms: u64,
    pub reason: LockReason,
}

/// Why the lock body's `blocked_until_ms` is set. Matches TS `error`
/// field domain minus `parse-error` (we never write that; corruption
/// surfaces as an unparseable body → treated as absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockReason {
    RateLimited,
    Timeout,
}

/// Exclusive lock guard. `Drop` releases the OS lock; on success paths
/// we overwrite the body with `blocked_until_ms = 0` before dropping so
/// concurrent readers see a "free" marker rather than a delete race.
pub struct UsageLock {
    file: File,
    #[allow(dead_code)]
    path: PathBuf,
}

impl UsageLock {
    /// Open + `try_lock_exclusive`. Returns `None` on any error
    /// (including contention). Callers fall back to stale cache.
    pub fn try_acquire(path: &Path) -> Option<Self> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .ok()?;
        file.try_lock_exclusive().ok()?;
        Some(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    /// Read + JSON-parse the body. `None` on empty / malformed / stale
    /// (mtime > 24h ago). Uses `&mut self` because seeking mutates the
    /// file handle's cursor.
    pub fn read_body(&mut self) -> Option<LockBody> {
        if self.is_stale() {
            return None;
        }
        self.file.seek(SeekFrom::Start(0)).ok()?;
        let mut raw = String::new();
        self.file.read_to_string(&mut raw).ok()?;
        if raw.trim().is_empty() {
            return None;
        }
        serde_json::from_str(&raw).ok()
    }

    /// Overwrite the body atomically-under-lock. Truncates then writes.
    /// Errors are swallowed to keep the fetch pipeline fail-closed.
    pub fn write(&mut self, blocked_until_ms: u64, reason: LockReason) {
        let body = LockBody {
            blocked_until_ms,
            reason,
        };
        let Ok(bytes) = serde_json::to_vec(&body) else {
            return;
        };
        let _ = self.file.set_len(0);
        let _ = self.file.seek(SeekFrom::Start(0));
        let _ = self.file.write_all(&bytes);
        let _ = self.file.flush();
    }

    fn is_stale(&self) -> bool {
        let Ok(meta) = self.file.metadata() else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        SystemTime::now()
            .duration_since(mtime)
            .map(|d| d > Duration::from_secs(STALE_LOCK_MAX_AGE_SECS))
            .unwrap_or(false)
    }
}

impl Drop for UsageLock {
    fn drop(&mut self) {
        // fs2's `unlock` is best-effort; the OS also releases on close.
        let _ = FileExt::unlock(&self.file);
    }
}

/// Lock file lives beside the cache file (design D2).
pub(super) fn usage_lock_path() -> Option<PathBuf> {
    let cache = super::cache::usage_cache_path()?;
    let parent = cache.parent()?;
    Some(parent.join("usage.lock"))
}

/// Build a synthesized `RenderUsageData` for use when a peer process
/// has stamped `blocked_until_ms` into the lock — i.e. we know we'd
/// hit a rate limit / timeout without asking the API.
pub(super) fn synthesize_backoff_data(reason: LockReason) -> RenderUsageData {
    let error = match reason {
        LockReason::RateLimited => UsageError::RateLimited,
        LockReason::Timeout => UsageError::Timeout,
    };
    RenderUsageData {
        error: Some(error),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_lock_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("usage.lock");
        (dir, path)
    }

    #[test]
    fn acquire_write_read_roundtrip() {
        let (_dir, path) = temp_lock_path();
        {
            let mut lock = UsageLock::try_acquire(&path).expect("acquire");
            lock.write(1_234_567, LockReason::RateLimited);
        }
        let mut lock2 = UsageLock::try_acquire(&path).expect("re-acquire after drop");
        let body = lock2.read_body().expect("body");
        assert_eq!(body.blocked_until_ms, 1_234_567);
        assert_eq!(body.reason, LockReason::RateLimited);
    }

    #[test]
    fn read_empty_body_returns_none() {
        let (_dir, path) = temp_lock_path();
        // Create empty file.
        std::fs::write(&path, b"").unwrap();
        let mut lock = UsageLock::try_acquire(&path).expect("acquire");
        assert!(lock.read_body().is_none());
    }

    #[test]
    fn read_malformed_body_returns_none() {
        let (_dir, path) = temp_lock_path();
        std::fs::write(&path, b"not-json {{{").unwrap();
        let mut lock = UsageLock::try_acquire(&path).expect("acquire");
        assert!(lock.read_body().is_none());
    }

    #[test]
    fn read_missing_field_returns_none() {
        let (_dir, path) = temp_lock_path();
        std::fs::write(&path, br#"{"blocked_until_ms": 1}"#).unwrap();
        let mut lock = UsageLock::try_acquire(&path).expect("acquire");
        assert!(lock.read_body().is_none());
    }

    #[test]
    fn read_unknown_reason_returns_none() {
        let (_dir, path) = temp_lock_path();
        std::fs::write(&path, br#"{"blocked_until_ms": 1, "reason": "made-up"}"#).unwrap();
        let mut lock = UsageLock::try_acquire(&path).expect("acquire");
        assert!(lock.read_body().is_none());
    }

    #[test]
    fn write_truncates_prior_longer_body() {
        let (_dir, path) = temp_lock_path();
        // Pre-populate with a long body.
        {
            let mut f = File::create(&path).unwrap();
            f.write_all(&vec![b'X'; 4096]).unwrap();
        }
        let mut lock = UsageLock::try_acquire(&path).expect("acquire");
        lock.write(42, LockReason::Timeout);
        drop(lock);

        let mut lock2 = UsageLock::try_acquire(&path).expect("re-acquire");
        let body = lock2.read_body().expect("body");
        assert_eq!(body.blocked_until_ms, 42);
        assert_eq!(body.reason, LockReason::Timeout);
        // File size should equal the fresh JSON — no trailing garbage.
        let meta = std::fs::metadata(&path).unwrap();
        let expected = serde_json::to_vec(&LockBody {
            blocked_until_ms: 42,
            reason: LockReason::Timeout,
        })
        .unwrap()
        .len() as u64;
        assert_eq!(meta.len(), expected);
    }

    #[test]
    fn stale_lock_body_ignored() {
        use std::fs::FileTimes;
        let (_dir, path) = temp_lock_path();
        // Write a real body.
        {
            let mut lock = UsageLock::try_acquire(&path).expect("acquire");
            lock.write(u64::MAX, LockReason::RateLimited);
        }
        // Backdate the mtime to 25h ago.
        let f = OpenOptions::new().write(true).open(&path).unwrap();
        let old = SystemTime::now() - Duration::from_secs(25 * 60 * 60);
        let times = FileTimes::new().set_modified(old).set_accessed(old);
        f.set_times(times).unwrap();
        drop(f);

        let mut lock = UsageLock::try_acquire(&path).expect("acquire");
        assert!(
            lock.read_body().is_none(),
            "stale lock (>24h mtime) must ignore body"
        );
    }

    #[test]
    fn contention_returns_none() {
        let (_dir, path) = temp_lock_path();
        let _held = UsageLock::try_acquire(&path).expect("first acquire");
        let second = UsageLock::try_acquire(&path);
        assert!(
            second.is_none(),
            "second acquire while first is held must return None"
        );
    }

    #[test]
    fn synthesize_backoff_carries_correct_error() {
        let d = synthesize_backoff_data(LockReason::RateLimited);
        assert_eq!(d.error, Some(UsageError::RateLimited));
        let d = synthesize_backoff_data(LockReason::Timeout);
        assert_eq!(d.error, Some(UsageError::Timeout));
    }
}
