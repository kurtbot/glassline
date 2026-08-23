//! Grok CLI adapter (issue #16). Ships as a Grok plugin — writes
//! `~/.grok/plugins/glassline/plugin.json` declaring slash commands
//! that a user activates via `grok plugin enable glassline`.
//!
//! Reference implementation for the "read `~/.grok/*.json` state and
//! print a status line" pattern is xiyouMc/grok-hud. glassline follows
//! the same shape: a plugin, three slash commands, an explicit
//! activation step the user runs after install.
//!
//! **Scope of P4a** (this file's initial landing):
//!   * `install()` / `uninstall()` — write / remove the plugin manifest.
//!     Fully implemented. `install` populates `post_install_hint` with
//!     the `grok plugin enable glassline` command the user must run
//!     next; `render_report` prints it under a `Next:` line.
//!   * `unsupported_widgets()` — Grok exposes context + model + tools
//!     via `~/.grok/signals.json`, but not the full Anthropic-shaped
//!     block/weekly quota concepts, so those widgets render as
//!     `(unavailable)` when the wizard summary is shown.
//!   * `read_context()` — deferred to P4b (joint refactor with P3b
//!     Codex render). Rendering FROM a Grok slash-command invocation
//!     needs signals.json / updates.jsonl / active_sessions.json
//!     parsers + main.rs dispatch refactor. See design v1.0 §4.4.
//!
//! **Manifest shape** — Grok's plugin format (per superagent-ai/grok-cli
//! docs) accepts `name` / `version` / `description` plus a
//! `slashCommands` array where each entry declares a `name` and an
//! executable `command`. Users invoke via `/<plugin>:<command>` in the
//! Grok REPL.
//!
//! **Backing-store detection**: `$GROK_HOME` env var wins; otherwise
//! `~/.grok`. Matches grok-hud's resolution.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::adapter::CliAdapter;
use crate::install::{InstallError, InstallOpts, InstallReport, Scope};

/// The Grok adapter. Zero-sized — behavior lives in the trait impl.
pub struct GrokAdapter;

impl CliAdapter for GrokAdapter {
    fn key(&self) -> &'static str {
        "grok"
    }

    fn display_name(&self) -> &'static str {
        "Grok"
    }

    fn install(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
        let grok_home = resolve_grok_home(opts.scope)?;
        let plugin_dir = grok_home.join("plugins").join("glassline");
        let plugin_json_path = plugin_dir.join("plugin.json");

        // Idempotency: if `plugin.json` already exists and points at
        // us, skip (unless --force). Mirrors CodexAdapter's shape.
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
                    // Even on the noop path we surface the enable
                    // hint — the user may have installed the manifest
                    // in a previous session but not yet run enable.
                    post_install_hint: Some(activate_hint()),
                });
            }
            return Err(InstallError::AlreadyConfigured {
                path: plugin_json_path,
                existing: existing_v,
            });
        }

        let plugin_manifest = build_plugin_manifest();

        if opts.dry_run {
            return Ok(InstallReport {
                path: plugin_json_path.clone(),
                previous: None,
                new: Some(plugin_manifest.clone()),
                wrote: false,
                seeded_config: None,
                post_install_hint: Some(activate_hint()),
            });
        }

        fs::create_dir_all(&plugin_dir).map_err(|e| InstallError::Io {
            path: plugin_dir.clone(),
            source: e,
        })?;
        write_json_atomic(&plugin_json_path, &plugin_manifest)?;

        Ok(InstallReport {
            path: plugin_json_path,
            previous: None,
            new: Some(plugin_manifest),
            wrote: true,
            seeded_config: None,
            post_install_hint: Some(activate_hint()),
        })
    }

    fn uninstall(&self, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
        let grok_home = resolve_grok_home(opts.scope)?;
        let plugin_dir = grok_home.join("plugins").join("glassline");
        let plugin_json_path = plugin_dir.join("plugin.json");

        if !plugin_json_path.exists() {
            return Ok(InstallReport {
                path: plugin_json_path,
                previous: None,
                new: None,
                wrote: false,
                seeded_config: None,
                post_install_hint: None,
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
                post_install_hint: None,
            });
        }

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
            post_install_hint: Some(
                "Run: grok plugin disable glassline (if it was enabled)".to_string(),
            ),
        })
    }

    fn unsupported_widgets(&self) -> &'static [&'static str] {
        // Grok's signals.json exposes model + context + active tool
        // but not the Anthropic-shaped block/weekly quota concepts.
        &[
            "block-timer",
            "block-reset-timer",
            "session-clock",
            "weekly-usage",
            "weekly-sonnet-usage",
            "weekly-opus-usage",
            "weekly-reset-timer",
        ]
    }

    fn read_context(
        &self,
        _stdin: &str,
    ) -> Result<glassline_core::render_context::RenderContext, String> {
        // P4b will parse ~/.grok/signals.json + updates.jsonl +
        // active_sessions.json and build a RenderContext. Until then
        // the adapter returns the stub message so users routed here
        // via GROK_HOME see a concrete "not yet" instead of silence.
        Err(crate::adapter::NOT_YET_IMPLEMENTED_MSG.to_string())
    }
}

/// Resolve `$GROK_HOME` → `~/.grok`. Grok plugins are user-scoped in
/// v1; `--project` is accepted but resolves to the user path (matches
/// Grok's own plugin model).
fn resolve_grok_home(_scope: Scope) -> Result<PathBuf, InstallError> {
    if let Some(env) = std::env::var_os("GROK_HOME") {
        let dir = PathBuf::from(env);
        fs::create_dir_all(&dir).map_err(|e| InstallError::Io {
            path: dir.clone(),
            source: e,
        })?;
        return Ok(dir);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or(InstallError::NoHome)?;
    let dir = PathBuf::from(home).join(".grok");
    fs::create_dir_all(&dir).map_err(|e| InstallError::Io {
        path: dir.clone(),
        source: e,
    })?;
    Ok(dir)
}

fn build_plugin_manifest() -> serde_json::Value {
    // Slash commands mirror grok-hud's shape:
    //   /glassline:status    — one-shot render of the current session
    //   /glassline:watch     — hint to run the render on a timer
    //   /glassline:configure — open the editor
    //
    // The `command` field points at `glassline` on PATH. On Windows
    // Grok invokes plugins via cmd.exe / PowerShell so PATH resolution
    // works either way. If a user later hits the render side and
    // finds nothing rendering (P4b will fix this), they'll get the
    // "no config yet" hint from the first-run placeholder.
    serde_json::json!({
        "name": "glassline",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Status line formatter for Grok — model, context, git, session, usage.",
        "slashCommands": [
            {
                "name": "status",
                "description": "Render current session status.",
                "command": "glassline"
            },
            {
                "name": "watch",
                "description": "Print watch-mode setup hint.",
                "command": "glassline",
                "args": ["--watch-hint"]
            },
            {
                "name": "configure",
                "description": "Open the interactive editor.",
                "command": "glassline-tui"
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

fn activate_hint() -> String {
    "Run: grok plugin enable glassline".to_string()
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

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "glassline-grok-{}-{}-{}",
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
        assert!(m.get("version").is_some());
        assert!(m.get("slashCommands").is_some());
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
    fn slash_commands_include_status_watch_configure() {
        let m = build_plugin_manifest();
        let cmds = m.get("slashCommands").and_then(|v| v.as_array()).unwrap();
        let names: Vec<&str> = cmds
            .iter()
            .filter_map(|c| c.get("name").and_then(|v| v.as_str()))
            .collect();
        assert!(names.contains(&"status"));
        assert!(names.contains(&"watch"));
        assert!(names.contains(&"configure"));
    }

    #[test]
    fn configure_command_points_at_glassline_tui() {
        let m = build_plugin_manifest();
        let cmds = m.get("slashCommands").and_then(|v| v.as_array()).unwrap();
        let configure = cmds
            .iter()
            .find(|c| c.get("name").and_then(|v| v.as_str()) == Some("configure"))
            .unwrap();
        assert_eq!(
            configure.get("command").and_then(|v| v.as_str()),
            Some("glassline-tui")
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
        assert_eq!(GrokAdapter.key(), "grok");
        assert_eq!(GrokAdapter.display_name(), "Grok");
    }

    #[test]
    fn adapter_unsupported_widgets_covers_weekly_and_block() {
        let ws = GrokAdapter.unsupported_widgets();
        assert!(ws.contains(&"block-timer"));
        assert!(ws.contains(&"weekly-usage"));
        assert!(ws.contains(&"session-clock"));
        // Grok has model + context so those must NOT be excluded.
        assert!(!ws.contains(&"model"));
        assert!(!ws.contains(&"context-percentage"));
    }

    #[test]
    fn activate_hint_mentions_grok_plugin_enable() {
        let hint = activate_hint();
        assert!(hint.contains("grok plugin enable"));
        assert!(hint.contains("glassline"));
    }
}
