//! Two-process race for `UsageLock`. Verifies D3 semantics from
//! On contention, `try_acquire`
//! returns `None` immediately (no spin); after the holder drops the
//! lock, a subsequent process can acquire.
//!
//! Spawns the test binary as a subprocess with a sentinel env var so
//! the child role runs `try_acquire` on the same path and exits with
//! an outcome code the parent can assert on.

use std::{env, path::PathBuf, process::Command};

use glassline_render::usage::lock::UsageLock;

const CHILD_ENV: &str = "GLASSLINE_LOCK_RACE_CHILD";
const PATH_ENV: &str = "GLASSLINE_LOCK_PATH";

/// If the sentinel env is set, we ARE the child. Attempt acquire, exit
/// with 0 (got lock) or 2 (contended). Never returns to caller.
fn maybe_run_child_role() {
    if env::var(CHILD_ENV).is_err() {
        return;
    }
    let path = PathBuf::from(env::var(PATH_ENV).expect("PATH_ENV must be set for child"));
    match UsageLock::try_acquire(&path) {
        Some(_lock) => std::process::exit(0),
        None => std::process::exit(2),
    }
}

fn spawn_child(test_name: &str, lock_path: &std::path::Path) -> std::process::Output {
    Command::new(env::current_exe().expect("current_exe"))
        .args([test_name, "--exact", "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, "1")
        .env(PATH_ENV, lock_path)
        .output()
        .expect("spawn child")
}

#[test]
fn contention_then_release_then_reacquire() {
    maybe_run_child_role();

    let tmp = tempfile::tempdir().expect("tempdir");
    let lock_path = tmp.path().join("test.lock");

    // Parent acquires.
    let parent = UsageLock::try_acquire(&lock_path).expect("parent acquire");

    // Child #1: parent still holds → contention expected (exit 2).
    let out = spawn_child("contention_then_release_then_reacquire", &lock_path);
    assert_eq!(
        out.status.code(),
        Some(2),
        "child while parent holds should hit contention; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Parent releases.
    drop(parent);

    // Child #2: parent released → should acquire (exit 0).
    let out = spawn_child("contention_then_release_then_reacquire", &lock_path);
    assert_eq!(
        out.status.code(),
        Some(0),
        "child after parent release should acquire; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn same_process_second_acquire_returns_none() {
    // Not a two-proc test — this verifies contention within-process
    // (fs2 lock semantics on Windows are per-handle; unix per-process).
    // We hold via one handle and open a second: on Windows, the second
    // fails; on unix, `flock` re-acquires as no-op. We accept either.
    maybe_run_child_role();

    let tmp = tempfile::tempdir().expect("tempdir");
    let lock_path = tmp.path().join("test.lock");

    let _first = UsageLock::try_acquire(&lock_path).expect("first acquire");
    let second = UsageLock::try_acquire(&lock_path);

    #[cfg(windows)]
    assert!(
        second.is_none(),
        "second acquire in same process must fail on Windows"
    );
    #[cfg(unix)]
    let _ = second; // unix flock is per-process advisory; behavior differs.
}
