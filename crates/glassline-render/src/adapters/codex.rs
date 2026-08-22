//! Codex CLI adapter (issue #15). Ships as a Codex plugin — writes
//! `~/.codex/plugins/glassline/plugin.json` + `hooks.json` so Codex
//! picks up glassline as an installed plugin.
//!
//! **Scope of P3a** (this file's initial landing):
//!   * `install()` / `uninstall()` — write / remove the plugin
//!     manifest. Fully implemented.
//!   * `unsupported_widgets()` — Codex has no five-hour block quota
//!     and its usage/session concepts differ, so those widgets render
//!     as `(unavailable)` when the wizard summary is shown.
//!   * `read_context()` — deferred to P3b. Rendering FROM a Codex
//!     invocation (reading `$CODEX_HOME/sessions/*.jsonl` rollout
//!     files, feature-detecting Codex's forward-compat `statusLine`
//!     hook per openai/codex#16921) is a bigger refactor of
//!     `main.rs`'s dispatch. Keeping this PR small enough to actually
//!     review. P3b lands the render side + the `read_context` trait
//!     method.
//!
//! **Manifest shape** — based on Codex's `plugin.json` schema (see
//! `codex-rs/skills/src/assets/samples/plugin-creator/references/plugin-json-spec.md`).
//! Minimal fields to be picked up: `name`, `version`, `description`,
//! `hooks`. Interface metadata (displayName, category, screenshots)
//! omitted — those are marketplace listing fields, not required for
//! a locally-installed plugin.
//!
//! **Hooks shape** — `hooks.json` declares a `PostToolUse` matcher
//! that fires `glassline` after every tool call. On the render side
//! (P3b), the CodexAdapter will read the latest rollout file and
//! print a status line to stdout. In the meantime the hook is inert
//! but declared, so a user running `codex plugin enable glassline`
//! sees glassline in their plugin list.
//!
//! **Backing-store detection**: `$CODEX_HOME` env var wins; otherwise
//! `$XDG_CONFIG_HOME/codex`; otherwise `~/.codex`. Matches Codex's own
//! resolution per `codex-rs/exec/src/cli.rs`.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::adapter::CliAdapter;
use crate::install::{InstallError, InstallOpts, InstallReport, Scope};

/// The Codex adapter. Zero-sized — behavior is entirely in the trait
/// impl.
pub struct CodexAdapter;

impl CliAdapter for CodexAdapter {
    fn key(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
        let codex_home = resolve_codex_home(opts.scope)?;
        let plugin_dir = codex_home.join("plugins").join("glassline");
        let plugin_json_path = plugin_dir.join("plugin.json");
        let hooks_json_path = plugin_dir.join("hooks.json");

        // Idempotency: if `plugin.json` already exists and points at
        // us, skip (unless --force). Mirrors the Claude adapter's
        // `is_glassline_entry` check.
        if plugin_json_path.exists() && !opts.force {
            let existing = fs::read_to_string(&plugin_json_path).map_err(|e| InstallError::Io {
                path: plugin_json_path.clone(),
                source: e,
            })?;
            let existing_v: serde_json::Value =
                serde_json::from_str(&existing).map_err(|e| InstallError::ParseExisting {
                    path: plugin_json_path.clone(),
                    source: e,
                })?;
            if is_glassline_plugin(&existing_v) {
                return Ok(InstallReport {
                    path: plugin_json_path,
                    previous: Some(existing_v.clone()),
                    new: Some(existing_v),
                    wrote: false,
                    seeded_config: None,
                });
            }
            // Some other plugin named `plugin.json` under our dir? That
            // shouldn't happen — the dir is namespaced by "glassline" —
            // but treat it as a foreign entry to be safe.
            return Err(InstallError::AlreadyConfigured {
                path: plugin_json_path,
                existing: existing_v,
            });
        }

        let plugin_manifest = build_plugin_manifest();
        let hooks_manifest = build_hooks_manifest();

        if opts.dry_run {
            return Ok(InstallReport {
                path: plugin_json_path.clone(),
                previous: None,
                new: Some(plugin_manifest.clone()),
                wrote: false,
                seeded_config: None,
            });
        }

        fs::create_dir_all(&plugin_dir).map_err(|e| InstallError::Io {
            path: plugin_dir.clone(),
            source: e,
        })?;
        write_json_atomic(&plugin_json_path, &plugin_manifest)?;
        write_json_atomic(&hooks_json_path, &hooks_manifest)?;

        Ok(InstallReport {
            path: plugin_json_path,
            previous: None,
            new: Some(plugin_manifest),
            wrote: true,
            seeded_config: None,
        })
    }

    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
        let codex_home = resolve_codex_home(opts.scope)?;
        let plugin_dir = codex_home.join("plugins").join("glassline");
        let plugin_json_path = plugin_dir.join("plugin.json");

        if !plugin_json_path.exists() {
            return Ok(InstallReport {
                path: plugin_json_path,
                previous: None,
                new: None,
                wrote: false,
                seeded_config: None,
            });
        }

        let existing_raw = fs::read_to_string(&plugin_json_path).map_err(|e| InstallError::Io {
            path: plugin_json_path.clone(),
            source: e,
        })?;
        let existing: serde_json::Value =
            serde_json::from_str(&existing_raw).map_err(|e| InstallError::ParseExisting {
                path: plugin_json_path.clone(),
                source: e,
            })?;

        if !opts.force && !is_glassline_plugin(&existing) {
            return Err(InstallError::NotGlassline {
                path: plugin_json_path,
                existing,
            });
        }

        if opts.dry_run {
            return Ok(InstallReport {
                path: plugin_json_path,
                previous: Some(existing),
                new: None,
                wrote: false,
                seeded_config: None,
            });
        }

        // Remove the whole plugin dir (plugin.json + hooks.json).
        fs::remove_dir_all(&plugin_dir).map_err(|e| InstallError::Io {
            path: plugin_dir.clone(),
            source: e,
        })?;

        Ok(InstallReport {
            path: plugin_json_path,
            previous: Some(existing),
            new: None,
            wrote: true,
            seeded_config: None,
        })
    }

    fn unsupported_widgets(&self) -> &'static [&'static str] {
        // Codex has no five-hour block quota concept and its
        // session/usage endpoints differ from Anthropic's. Widgets
        // that key on those fields render as `(unavailable)` when
        // dispatched through this adapter.
        &["block-timer", "block-reset-timer", "session-usage"]
    }
}

/// Resolve `$CODEX_HOME` → `$XDG_CONFIG_HOME/codex` → `~/.codex`.
/// The `--project` scope is not meaningful for Codex plugins today
/// (Codex plugins live under user-home, not per-project) — we accept
/// the flag and always resolve to the user-scoped location, matching
/// Codex's own plugin model.
fn resolve_codex_home(_scope: Scope) -> Result<PathBuf, InstallError> {
    if let Some(env) = std::env::var_os("CODEX_HOME") {
        let dir = PathBuf::from(env);
        fs::create_dir_all(&dir).map_err(|e| InstallError::Io {
            path: dir.clone(),
            source: e,
        })?;
        return Ok(dir);
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let dir = PathBuf::from(xdg).join("codex");
        fs::create_dir_all(&dir).map_err(|e| InstallError::Io {
            path: dir.clone(),
            source: e,
        })?;
        return Ok(dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or(InstallError::NoHome)?;
    let dir = PathBuf::from(home).join(".codex");
    fs::create_dir_all(&dir).map_err(|e| InstallError::Io {
        path: dir.clone(),
        source: e,
    })?;
    Ok(dir)
}

fn build_plugin_manifest() -> serde_json::Value {
    serde_json::json!({
        "name": "glassline",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Status line formatter for Codex — model, context, git, session, usage.",
        "hooks": "./hooks.json",
        "interface": {
            "displayName": "glassline",
            "shortDescription": "Rich status line for Codex sessions.",
            "developerName": "kurtbot",
            "category": "Productivity",
        }
    })
}

fn build_hooks_manifest() -> serde_json::Value {
    // P3a: declare a PostToolUse hook targeting our binary. Once P3b
    // ships and CodexAdapter::read_context can render from rollout
    // files, this hook will produce a visible status line after every
    // tool call. In the meantime the hook is registered but inert
    // (glassline reads empty stdin, prints a placeholder).
    //
    // The Windows override runs the same binary via PowerShell to
    // sidestep Codex's Windows execution semantics — matches the
    // guarded-command shape in `hook_config.rs`.
    serde_json::json!({
        "PostToolUse": [
            {
                "matcher": "*",
                "hooks": [
                    {
                        "type": "command",
                        "command": "glassline",
                        "commandWindows": "glassline.exe",
                        "timeout": 5
                    }
                ]
            }
        ]
    })
}

fn is_glassline_plugin(value: &serde_json::Value) -> bool {
    value
        .get("name")
        .and_then(|v| v.as_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("glassline"))
}

fn write_json_atomic(path: &Path, value: &serde_json::Value) -> Result<(), InstallError> {
    let parent = path.parent().ok_or_else(|| InstallError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other("json path has no parent directory"),
    })?;
    let temp = parent.join(format!(
        ".{}.glassline-tmp",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("json")
    ));
    {
        let mut f = fs::File::create(&temp).map_err(|e| InstallError::Io {
            path: temp.clone(),
            source: e,
        })?;
        let bytes = serde_json::to_vec_pretty(value).map_err(|e| InstallError::ParseExisting {
            path: temp.clone(),
            source: e,
        })?;
        f.write_all(&bytes).map_err(|e| InstallError::Io {
            path: temp.clone(),
            source: e,
        })?;
        f.write_all(b"\n").map_err(|e| InstallError::Io {
            path: temp.clone(),
            source: e,
        })?;
    }
    fs::rename(&temp, path).map_err(|e| InstallError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Tempdir helper with CODEX_HOME override baked in — writes
    /// happen against the temp path, not the real ~/.codex.
    ///
    /// Because tests run with `unsafe_code = deny` at the workspace
    /// level, we can't set env vars from the test. Instead we exercise
    /// the pure functions directly (build_plugin_manifest,
    /// write_json_atomic, is_glassline_plugin) and skip the env-driven
    /// resolve_codex_home path — that's covered indirectly by the
    /// integration test in P3b when we can spawn the binary with an
    /// env override.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "glassline-codex-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0),
            ));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir_all(&base).unwrap();
            Self(base)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn plugin_manifest_has_required_fields() {
        let m = build_plugin_manifest();
        assert_eq!(m.get("name").and_then(|v| v.as_str()), Some("glassline"));
        assert!(m.get("version").is_some(), "manifest needs version");
        assert!(m.get("hooks").is_some(), "manifest needs hooks reference");
    }

    #[test]
    fn plugin_manifest_version_matches_crate() {
        let m = build_plugin_manifest();
        assert_eq!(
            m.get("version").and_then(|v| v.as_str()),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn hooks_manifest_declares_post_tool_use_targeting_glassline() {
        let h = build_hooks_manifest();
        let post = h.get("PostToolUse").and_then(|v| v.as_array()).unwrap();
        assert_eq!(post.len(), 1);
        let hook = &post[0].get("hooks").and_then(|v| v.as_array()).unwrap()[0];
        assert_eq!(
            hook.get("command").and_then(|v| v.as_str()),
            Some("glassline")
        );
    }

    #[test]
    fn is_glassline_plugin_matches_our_manifest() {
        assert!(is_glassline_plugin(&build_plugin_manifest()));
        assert!(is_glassline_plugin(&json!({"name": "glassline"})));
        assert!(is_glassline_plugin(&json!({"name": "GLASSLINE"})));
        assert!(!is_glassline_plugin(&json!({"name": "other-tool"})));
        assert!(!is_glassline_plugin(&json!({})));
    }

    #[test]
    fn write_json_atomic_lands_valid_json() {
        let td = TempDir::new("write");
        let path = td.0.join("nested").join("plugin.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_json_atomic(&path, &build_plugin_manifest()).unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            parsed.get("name").and_then(|v| v.as_str()),
            Some("glassline")
        );
    }

    #[test]
    fn adapter_key_and_display() {
        assert_eq!(CodexAdapter.key(), "codex");
        assert_eq!(CodexAdapter.display_name(), "Codex");
    }

    #[test]
    fn adapter_unsupported_widgets_includes_block_widgets() {
        let ws = CodexAdapter.unsupported_widgets();
        assert!(ws.contains(&"block-timer"));
        assert!(ws.contains(&"block-reset-timer"));
        assert!(ws.contains(&"session-usage"));
    }
}
