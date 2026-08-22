//! CLI adapter framework — one trait, one compile-time registry, one
//! per-CLI install path.
//!
//! Every coding CLI glassline knows how to wire into is represented as
//! a static [`CliAdapter`] entry in [`REGISTRY`]. Each adapter owns:
//!   * the `--for <slug>` argument on `glassline install`
//!   * the install / uninstall implementation for that slug (which
//!     writes wherever the target CLI expects — Claude's
//!     `settings.json` JSON key, Codex's `plugin.json`, Grok's plugin
//!     manifest + `grok plugin enable` hint, and so on)
//!   * the set of widget kinds that render as `(unavailable)` on that
//!     CLI, so the wizard can warn the user pre-install
//!
//! Rendering itself stays adapter-agnostic — every adapter's
//! `read_context` (added in P3 when Codex/Grok arrive) ends by
//! producing a `RenderContext`, which the existing widget catalog +
//! renderer consume without knowing which CLI produced it.
//!
//! **P1 scope note.** This module ships the trait, the REGISTRY, and
//! [`ClaudeAdapter`] as a thin wrapper around the existing
//! [`crate::install::run_install`] / `run_uninstall`. The
//! `read_context` method is deferred to P3 — Claude's read path is
//! still handled directly in `main.rs`, and the trait grows the
//! method when there's a second adapter to give it meaning.

use phf::phf_map;

use crate::install::{InstallError, InstallOpts, InstallReport, run_install, run_uninstall};

/// The trait every coding-CLI adapter implements.
///
/// All methods are `&self`-driven and take a concrete `InstallOpts`
/// so callers can share one `&'static dyn CliAdapter` reference from
/// [`REGISTRY`] without boxing per invocation. The trait is
/// `Send + Sync` so it composes with the workspace's future
/// concurrency model (currently synchronous, but the constraint is
/// zero cost).
pub trait CliAdapter: Send + Sync {
    /// Stable slug matching the [`REGISTRY`] key and the value the
    /// user passes to `glassline install --for <slug>`. Never rename
    /// once shipped — release compatibility depends on this string.
    fn key(&self) -> &'static str;

    /// Human-readable name for wizard prompts and diagnostics. May
    /// change between releases.
    fn display_name(&self) -> &'static str;

    /// Install glassline into this CLI's status-line / plugin surface.
    /// Returns a fully-populated [`InstallReport`] so callers can
    /// print a summary and downstream code (wizard, tests) can assert
    /// on the concrete outcome.
    fn install(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError>;

    /// Remove glassline from this CLI's surface. Symmetric with
    /// [`install`](Self::install).
    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError>;

    /// Widget kinds that will render as `(unavailable)` on this CLI —
    /// e.g. Codex has no `session-usage` concept, so a settings.json
    /// with a `session-usage` widget in it renders that widget blank
    /// when Codex invokes glassline. The wizard summary modal lists
    /// these so users know pre-install.
    ///
    /// Empty slice = "every widget renders". `ClaudeAdapter` returns
    /// `&[]` because Claude Code drives the full canonical catalog.
    fn unsupported_widgets(&self) -> &'static [&'static str];
}

/// The `--for <slug>` registry. `phf::Map` keeps the lookup O(1) and
/// the whole table statically allocated — one branch of the render
/// binary's install dispatch consumes this.
///
/// P1 ships with `claude` only. P3 adds `codex`; P4 adds `grok`.
pub static REGISTRY: phf::Map<&'static str, &'static dyn CliAdapter> = phf_map! {
    "claude" => &ClaudeAdapter as &'static dyn CliAdapter,
    "codex"  => &crate::adapters::codex::CodexAdapter as &'static dyn CliAdapter,
};

/// Stable iteration order for callers that need to enumerate every
/// adapter (diagnostics screen, `--help` output). `phf::Map`'s own
/// iteration order is technically deterministic but not source-order,
/// so callers that care use this instead.
pub const REGISTRY_ORDER: &[&str] = &["claude", "codex"];

/// Choose an adapter based on the caller's environment. Called by
/// the render binary when stdin arrives without an explicit `--for`
/// hint — we route through the env var the surrounding CLI would
/// have set.
///
/// P1 always returns [`ClaudeAdapter`] because it's the only entry.
/// P3 adds branches for `CODEX_HOME`; P4 adds `GROK_HOME`. The
/// fallback is always Claude to preserve backcompat with anyone
/// piping raw `StatusJSON` from a wrapper.
#[must_use]
pub fn env_var_dispatch() -> &'static dyn CliAdapter {
    // P3/P4 will grow branches here — the fallback stays Claude.
    &ClaudeAdapter
}

/// The reference implementation. Wraps the existing
/// [`crate::install::run_install`] / `run_uninstall` unchanged so
/// this refactor is byte-identical for the `glassline install`
/// bare-flag (no `--for`) call path.
pub struct ClaudeAdapter;

impl CliAdapter for ClaudeAdapter {
    fn key(&self) -> &'static str {
        "claude"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
        run_install(opts)
    }

    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
        run_uninstall(opts)
    }

    fn unsupported_widgets(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_adapter_key_is_claude() {
        assert_eq!(ClaudeAdapter.key(), "claude");
    }

    #[test]
    fn claude_adapter_display_name_is_human_readable() {
        assert_eq!(ClaudeAdapter.display_name(), "Claude Code");
    }

    #[test]
    fn claude_adapter_has_no_unsupported_widgets() {
        assert!(ClaudeAdapter.unsupported_widgets().is_empty());
    }

    #[test]
    fn registry_contains_claude() {
        let adapter = REGISTRY.get("claude").expect("claude should be registered");
        assert_eq!(adapter.key(), "claude");
    }

    #[test]
    fn registry_lookup_of_unknown_slug_returns_none() {
        assert!(REGISTRY.get("nope").is_none());
        assert!(REGISTRY.get("").is_none());
    }

    #[test]
    fn registry_order_matches_registry_keys() {
        // Guardrail: if REGISTRY grows a new entry, REGISTRY_ORDER
        // must grow with it. Otherwise enumeration silently drops
        // the newcomer.
        for key in REGISTRY_ORDER {
            assert!(
                REGISTRY.contains_key(key),
                "REGISTRY_ORDER references missing key {key:?}"
            );
        }
        assert_eq!(REGISTRY.len(), REGISTRY_ORDER.len());
    }

    #[test]
    fn env_var_dispatch_returns_claude_in_p1() {
        // Documented contract — P3/P4 flip this when they add
        // CODEX_HOME / GROK_HOME branches. Until then, the fallback
        // is Claude and that's what P1 asserts.
        assert_eq!(env_var_dispatch().key(), "claude");
    }

    #[test]
    fn registry_contains_codex_after_p3a() {
        let adapter = REGISTRY.get("codex").expect("codex should be registered");
        assert_eq!(adapter.key(), "codex");
        assert_eq!(adapter.display_name(), "Codex");
    }
}
