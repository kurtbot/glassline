//! Placeholder for the TS ccstatusline shim.
//!
//! The shim (impl plan T-0.11) invokes the pinned TS ccstatusline via `bunx`,
//! feeds a StatusJson payload on stdin, and captures the raw ANSI stdout.
//! Because CI runners may not have bun installed, the real invocation is
//! wrapped: callers are expected to first probe [`ts_shim_available`] and
//! either invoke [`run_ts_ccstatusline`] or fall back to a pre-recorded cache.
//!
//! P0 ships only the surface + probing — the actual `bunx` invocation lands
//! in P1 alongside the parity harness.

use std::path::PathBuf;

/// Where a cached TS output is stored on disk (`.ansi` sidecar per fixture).
#[must_use]
pub fn cached_output_path(cache_root: &std::path::Path, fixture_name: &str) -> PathBuf {
    cache_root.join(format!("{fixture_name}.ansi"))
}

/// Whether the current environment has `bun` on `PATH`.
#[must_use]
pub fn ts_shim_available() -> bool {
    which_command("bun").is_some() || which_command("bunx").is_some()
}

/// Placeholder — always returns `None` in P0. P1 replaces this with a real
/// `Command::new("bunx").arg("ccstatusline@2.2.27")` invocation.
#[must_use]
pub fn run_ts_ccstatusline(_payload_json: &str) -> Option<String> {
    None
}

fn which_command(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let extensions = std::env::var_os("PATHEXT")
        .map(|v| {
            v.to_string_lossy()
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let base = dir.join(name);
        if base.is_file() {
            return Some(base);
        }
        for ext in &extensions {
            let candidate = dir.join(format!("{name}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_output_path_composes_correctly() {
        let root = std::path::Path::new("/tmp/cache");
        let path = cached_output_path(root, "minimal");
        assert!(path.ends_with("minimal.ansi"));
    }

    #[test]
    fn ts_shim_probing_does_not_panic() {
        // Doesn't matter whether it's available; just that the check runs.
        let _ = ts_shim_available();
    }
}
