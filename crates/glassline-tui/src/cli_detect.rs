//! Compile-time registry of coding CLIs glassline knows how to wire
//! into, plus a cheap read-only detection pass that returns which of
//! them are present on the current machine.
//!
//! Callers ([`snapshot`]) get a stable-ordered `Vec` of
//! `(&CliCandidate, DetectResult)` tuples — one entry per REGISTRY
//! candidate. The first-run wizard takes one snapshot at push-time
//! (never re-polls during the session) and drives its picker UI from
//! it; diagnostics prints the same snapshot verbatim.
//!
//! Adding a new CLI: register a `CliCandidate` in the REGISTRY
//! `phf_map!` with a `detect: fn() -> DetectResult` that combines
//! marker-directory-check and `which::which(binary)`. Marker-dir alone
//! is a weak signal (glassline itself creates `~/.claude/` on
//! `install --user`), so the OR-with-binary check keeps false
//! positives out on second-run.

use std::path::PathBuf;

use phf::phf_map;

/// One entry in the CLI detection registry.
#[derive(Debug)]
pub struct CliCandidate {
    /// Stable slug. Matches the `--for <slug>` argument on
    /// `glassline install` (design §4.5). Never change once shipped.
    pub key: &'static str,
    /// Human-readable name for the wizard picker + diagnostics.
    pub display_name: &'static str,
    /// Path snippet shown in the picker to describe where install
    /// would land. Purely cosmetic — actual install path resolution
    /// happens in `glassline-render`'s install module.
    pub install_hint: &'static str,
    /// Detection callback. Must be cheap, read-only, and side-effect
    /// free. Called once per snapshot.
    pub detect: fn() -> DetectResult,
}

/// Outcome of a single per-CLI detection probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectResult {
    /// The CLI is present. `evidence` is the marker directory or
    /// binary path that satisfied the probe — surfaced in diagnostics
    /// so users can debug why detection fired.
    Installed { evidence: PathBuf },
    /// Probe ran and found nothing.
    NotInstalled,
    /// Probe cannot say for sure — treated as `NotInstalled` by the
    /// picker (won't offer the CLI) but rendered distinctly in
    /// diagnostics so we know it's a placeholder awaiting an adapter.
    Unknown,
}

/// Iteration order matches insertion order below. The wizard picker
/// and diagnostics both rely on this being deterministic so screens
/// don't reshuffle between renders.
///
/// Order rationale: `claude` first because it's the reference
/// implementation and the only shipped adapter as of this module;
/// `codex` next (issue #15); `grok` last (issue #16, placeholder).
pub static REGISTRY: phf::Map<&'static str, CliCandidate> = phf_map! {
    "claude" => CliCandidate {
        key: "claude",
        display_name: "Claude Code",
        install_hint: "~/.claude/settings.json",
        detect: detect_claude,
    },
    "codex" => CliCandidate {
        key: "codex",
        display_name: "Codex",
        install_hint: "~/.codex/",
        detect: detect_codex,
    },
    "grok" => CliCandidate {
        key: "grok",
        display_name: "Grok",
        install_hint: "(adapter not registered)",
        detect: detect_grok,
    },
};

/// Insertion-order iteration of REGISTRY. Kept as a `&[&str]` so the
/// snapshot walker doesn't depend on `phf::Map`'s Debug-visible order.
const REGISTRY_ORDER: &[&str] = &["claude", "codex", "grok"];

/// Run every candidate's detection function once, in REGISTRY_ORDER.
/// Returns a Vec so callers can hold the results across event ticks
/// without re-polling.
#[must_use]
pub fn snapshot() -> Vec<(&'static CliCandidate, DetectResult)> {
    REGISTRY_ORDER
        .iter()
        .filter_map(|key| REGISTRY.get(key).map(|c| (c, (c.detect)())))
        .collect()
}

fn detect_claude() -> DetectResult {
    check_marker_or_binary(".claude", "claude")
}

fn detect_codex() -> DetectResult {
    check_marker_or_binary(".codex", "codex")
}

fn detect_grok() -> DetectResult {
    // Adapter unshipped (issue #16). Placeholder so the picker's
    // registry has an entry to render as "(not detected)".
    DetectResult::Unknown
}

/// Shared marker-dir + binary-presence probe. Delegates to
/// [`check_marker_or_binary_in`] with the real home dir. Split for
/// testability — tests inject a temp path without needing to mutate
/// `HOME` / `USERPROFILE` (workspace lints deny unsafe, and env-var
/// setters are unsafe in Rust 2024).
///
/// Ordering: marker-dir first because a filesystem stat is cheaper
/// than a full `PATH` scan on Windows.
fn check_marker_or_binary(marker: &str, binary: &str) -> DetectResult {
    check_marker_or_binary_in(home_dir(), marker, binary)
}

fn check_marker_or_binary_in(home: Option<PathBuf>, marker: &str, binary: &str) -> DetectResult {
    if let Some(home) = home {
        let dir = home.join(marker);
        if dir.is_dir() {
            return DetectResult::Installed { evidence: dir };
        }
    }
    if let Ok(path) = which::which(binary) {
        return DetectResult::Installed { evidence: path };
    }
    DetectResult::NotInstalled
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Tempdir helper that owns a unique subdirectory and cleans up
    /// on drop. Does NOT touch env vars — tests call the `_in`
    /// helpers directly with `Some(self.dir.clone())`.
    struct TempHome {
        dir: PathBuf,
    }

    impl TempHome {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "glassline-cli-detect-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0),
            ));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base).unwrap();
            Self { dir: base }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn marker_dir_detected_as_installed() {
        let th = TempHome::new("claude-marker");
        fs::create_dir_all(th.dir.join(".claude")).unwrap();
        match check_marker_or_binary_in(
            Some(th.dir.clone()),
            ".claude",
            "definitely-not-a-real-binary-xyz",
        ) {
            DetectResult::Installed { evidence } => {
                assert!(evidence.ends_with(".claude"), "evidence: {evidence:?}");
            }
            other => panic!("expected Installed via marker dir, got {other:?}"),
        }
    }

    #[test]
    fn missing_marker_and_binary_returns_not_installed() {
        let th = TempHome::new("nothing");
        // Marker dir does not exist under `th.dir`; binary name is
        // synthetic so `which::which` returns Err on every runner.
        assert_eq!(
            check_marker_or_binary_in(
                Some(th.dir.clone()),
                ".definitely-not-real",
                "definitely-not-a-real-binary-xyz",
            ),
            DetectResult::NotInstalled
        );
    }

    #[test]
    fn none_home_falls_through_to_binary_check() {
        // Home path unavailable (e.g. a bare Docker container) —
        // detection still runs the binary probe and returns
        // NotInstalled for a synthetic binary name.
        assert_eq!(
            check_marker_or_binary_in(None, ".claude", "definitely-not-a-real-binary-xyz"),
            DetectResult::NotInstalled
        );
    }

    #[test]
    fn grok_placeholder_returns_unknown() {
        assert_eq!(detect_grok(), DetectResult::Unknown);
    }

    #[test]
    fn snapshot_is_registry_order_deterministic() {
        let snap = snapshot();
        let keys: Vec<&str> = snap.iter().map(|(c, _)| c.key).collect();
        assert_eq!(keys, vec!["claude", "codex", "grok"]);
    }

    #[test]
    fn snapshot_returns_one_result_per_registry_entry() {
        let snap = snapshot();
        assert_eq!(snap.len(), REGISTRY_ORDER.len());
    }

    #[test]
    fn registry_order_matches_registry_keys() {
        // Guardrail: if REGISTRY grows a new entry, REGISTRY_ORDER
        // must grow with it — otherwise `snapshot` silently drops
        // the newcomer.
        for k in REGISTRY_ORDER {
            assert!(
                REGISTRY.contains_key(k),
                "REGISTRY_ORDER references missing key {k:?}"
            );
        }
        assert_eq!(REGISTRY.len(), REGISTRY_ORDER.len());
    }
}
