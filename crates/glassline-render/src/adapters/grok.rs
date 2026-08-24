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

use glassline_core::{
    render_context::{RenderContext, TokenMetrics},
    status_json::{ContextWindow, ModelInfo, StatusJson, Workspace},
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

    fn read_context(&self, _stdin: &str) -> Result<RenderContext, String> {
        // Grok invokes plugins as slash commands with no piped stdin —
        // we ignore stdin and read state from `~/.grok/*.json`.
        // grok-hud reference: signals.json is the primary source for
        // model + context; updates.jsonl gives the active tool.
        //
        // signals.json is the mandatory source. Missing it means Grok
        // isn't in a state we can render from — error clearly rather
        // than silently return empty context.
        let grok_home =
            resolve_grok_home_readonly().map_err(|e| format!("resolve GROK_HOME: {e}"))?;
        let signals_path = grok_home.join("signals.json");
        if !signals_path.exists() {
            return Err(format!("no signals.json at {}", signals_path.display()));
        }
        let signals = parse_signals(&signals_path)
            .map_err(|e| format!("parse {}: {e}", signals_path.display()))?;
        // updates.jsonl gives us the active tool if present. It's
        // optional — missing file is not fatal.
        let updates_path = grok_home.join("updates.jsonl");
        let active_tool = if updates_path.exists() {
            latest_active_tool(&updates_path).unwrap_or(None)
        } else {
            None
        };
        Ok(signals_to_context(signals, active_tool))
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

// --- read_context helpers -------------------------------------------------

/// Read-only sibling of `resolve_grok_home` — doesn't try to create
/// the directory. Used by `read_context` where we want to look for
/// existing signals files; refusing to run when the dir is missing
/// is the right behavior.
fn resolve_grok_home_readonly() -> Result<PathBuf, String> {
    if let Some(env) = std::env::var_os("GROK_HOME") {
        return Ok(PathBuf::from(env));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| "no HOME / USERPROFILE / GROK_HOME".to_string())?;
    Ok(PathBuf::from(home).join(".grok"))
}

/// Fields we extract from `signals.json`. Permissive shape — Grok's
/// exact schema isn't publicly documented, so we read whatever
/// canonical names grok-hud uses and fall back to reasonable
/// alternatives. Missing fields stay None.
#[derive(Debug, Default, Clone)]
pub(crate) struct GrokSignals {
    pub model_id: Option<String>,
    pub model_display_name: Option<String>,
    pub context_used_tokens: Option<u64>,
    pub context_window_size: Option<u64>,
    pub context_used_percentage: Option<f64>,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
}

pub(crate) fn parse_signals(path: &Path) -> Result<GrokSignals, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let mut out = GrokSignals::default();

    // Model — try several field names Grok might use.
    if let Some(m) = value.get("model") {
        if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
            out.model_id = Some(id.to_string());
        }
        if let Some(name) = m.get("display_name").or_else(|| m.get("name"))
            && let Some(s) = name.as_str()
        {
            out.model_display_name = Some(s.to_string());
        }
        if let Some(s) = m.as_str() {
            out.model_id.get_or_insert(s.to_string());
        }
    }
    if let Some(name) = value.get("modelName").and_then(|v| v.as_str()) {
        out.model_display_name.get_or_insert(name.to_string());
    }

    // Context.
    if let Some(ctx) = value.get("context").or_else(|| value.get("contextWindow")) {
        if let Some(v) = ctx.get("used").and_then(u64_of) {
            out.context_used_tokens = Some(v);
        }
        if let Some(v) = ctx
            .get("size")
            .or_else(|| ctx.get("window"))
            .or_else(|| ctx.get("total"))
            .and_then(u64_of)
        {
            out.context_window_size = Some(v);
        }
        if let Some(v) = ctx.get("percentage").and_then(|x| x.as_f64()) {
            out.context_used_percentage = Some(v);
        }
    }
    // Top-level fallbacks (some emitters write context_used_tokens
    // directly on the root object).
    if let Some(v) = value.get("contextUsedTokens").and_then(u64_of) {
        out.context_used_tokens.get_or_insert(v);
    }
    if let Some(v) = value.get("contextWindowSize").and_then(u64_of) {
        out.context_window_size.get_or_insert(v);
    }

    if let Some(s) = value.get("session_id").and_then(|v| v.as_str()) {
        out.session_id = Some(s.to_string());
    } else if let Some(s) = value.get("sessionId").and_then(|v| v.as_str()) {
        out.session_id = Some(s.to_string());
    }
    if let Some(s) = value.get("cwd").and_then(|v| v.as_str()) {
        out.cwd = Some(s.to_string());
    }
    Ok(out)
}

/// Tail of `updates.jsonl`. Each line is a tool-invocation event.
/// Returns the `tool_name` of the most recent event, or None if the
/// file is empty / malformed. Errors reading the file bubble up as
/// `Err`; per-line JSON parse failures are silently skipped.
pub(crate) fn latest_active_tool(path: &Path) -> Result<Option<String>, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    for line in raw.lines().rev() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(name) = v.get("tool_name").and_then(|x| x.as_str()) {
            return Ok(Some(name.to_string()));
        }
        if let Some(name) = v.get("toolName").and_then(|x| x.as_str()) {
            return Ok(Some(name.to_string()));
        }
    }
    Ok(None)
}

fn u64_of(v: &serde_json::Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub(crate) fn signals_to_context(
    signals: GrokSignals,
    _active_tool: Option<String>,
) -> RenderContext {
    let context_window = if signals.context_used_tokens.is_some()
        || signals.context_window_size.is_some()
        || signals.context_used_percentage.is_some()
    {
        let pct = signals.context_used_percentage.or_else(|| {
            match (signals.context_used_tokens, signals.context_window_size) {
                (Some(used), Some(size)) if size > 0 => Some((used as f64 / size as f64) * 100.0),
                _ => None,
            }
        });
        Some(ContextWindow {
            context_window_size: signals.context_window_size.map(|v| v as f64),
            total_input_tokens: signals.context_used_tokens.map(|v| v as f64),
            total_output_tokens: None,
            current_usage: None,
            used_percentage: pct,
            remaining_percentage: pct.map(|p| 100.0 - p),
            usable_percentage: None,
        })
    } else {
        None
    };

    let model = if signals.model_id.is_some() || signals.model_display_name.is_some() {
        Some(ModelInfo::Full {
            id: signals.model_id,
            display_name: signals.model_display_name,
        })
    } else {
        None
    };

    let workspace = signals.cwd.clone().map(|cwd| Workspace {
        current_dir: Some(cwd.clone()),
        project_dir: Some(cwd),
        repo: None,
    });

    let status_json = StatusJson {
        session_id: signals.session_id,
        session_name: None,
        transcript_path: None,
        cwd: signals.cwd,
        model,
        workspace,
        version: None,
        output_style: None,
        effort: None,
        cost: None,
        context_window,
        vim: None,
        worktree: None,
        rate_limits: None,
        hook_event_name: None,
        extras: std::collections::BTreeMap::new(),
    };

    let token_metrics = signals.context_used_tokens.map(|used| TokenMetrics {
        input: used,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
        context_length: used,
    });

    RenderContext {
        data: Some(status_json),
        token_metrics,
        now_ms: now_ms(),
        ..RenderContext::default()
    }
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

    // --- signals parser tests -----------------------------------------

    #[test]
    fn parse_signals_extracts_nested_model_and_context() {
        let td = TempDir::new("signals-full");
        let path = td.0.join("signals.json");
        fs::write(
            &path,
            json!({
                "model": {"id": "grok-4-fast", "display_name": "Grok 4 Fast"},
                "context": {"used": 12000, "size": 128000},
                "session_id": "sess_xyz",
                "cwd": "/tmp/g"
            })
            .to_string(),
        )
        .unwrap();
        let s = parse_signals(&path).unwrap();
        assert_eq!(s.model_id.as_deref(), Some("grok-4-fast"));
        assert_eq!(s.model_display_name.as_deref(), Some("Grok 4 Fast"));
        assert_eq!(s.context_used_tokens, Some(12000));
        assert_eq!(s.context_window_size, Some(128000));
        assert_eq!(s.session_id.as_deref(), Some("sess_xyz"));
        assert_eq!(s.cwd.as_deref(), Some("/tmp/g"));
    }

    #[test]
    fn parse_signals_tolerates_top_level_context_fields() {
        let td = TempDir::new("signals-flat");
        let path = td.0.join("signals.json");
        fs::write(
            &path,
            json!({
                "modelName": "Grok Code",
                "contextUsedTokens": 5000,
                "contextWindowSize": 64000
            })
            .to_string(),
        )
        .unwrap();
        let s = parse_signals(&path).unwrap();
        assert_eq!(s.model_display_name.as_deref(), Some("Grok Code"));
        assert_eq!(s.context_used_tokens, Some(5000));
        assert_eq!(s.context_window_size, Some(64000));
    }

    #[test]
    fn parse_signals_handles_missing_optional_fields() {
        let td = TempDir::new("signals-sparse");
        let path = td.0.join("signals.json");
        fs::write(&path, "{}").unwrap();
        let s = parse_signals(&path).unwrap();
        assert!(s.model_id.is_none());
        assert!(s.context_used_tokens.is_none());
    }

    #[test]
    fn latest_active_tool_returns_most_recent_event() {
        let td = TempDir::new("updates");
        let path = td.0.join("updates.jsonl");
        let events = [
            json!({"tool_name": "read"}).to_string(),
            json!({"tool_name": "write"}).to_string(),
            json!({"tool_name": "bash"}).to_string(),
        ];
        fs::write(&path, events.join("\n")).unwrap();
        assert_eq!(latest_active_tool(&path).unwrap().as_deref(), Some("bash"));
    }

    #[test]
    fn latest_active_tool_skips_malformed_lines_and_returns_none_if_all_bad() {
        let td = TempDir::new("updates-bad");
        let path = td.0.join("updates.jsonl");
        fs::write(&path, "not-json\n\nalso-not-json").unwrap();
        assert_eq!(latest_active_tool(&path).unwrap(), None);
    }

    #[test]
    fn signals_to_context_computes_percentage_when_missing() {
        let signals = GrokSignals {
            model_id: Some("grok-4".to_string()),
            context_used_tokens: Some(50_000),
            context_window_size: Some(200_000),
            context_used_percentage: None,
            ..Default::default()
        };
        let ctx = signals_to_context(signals, None);
        let status = ctx.data.expect("status");
        let cw = status.context_window.expect("context_window");
        // 50k / 200k = 25%
        assert_eq!(cw.used_percentage, Some(25.0));
        assert_eq!(cw.remaining_percentage, Some(75.0));
    }

    #[test]
    fn signals_to_context_no_context_returns_none_context_window() {
        let signals = GrokSignals {
            model_id: Some("grok-4".to_string()),
            ..Default::default()
        };
        let ctx = signals_to_context(signals, None);
        let status = ctx.data.expect("status");
        assert!(status.context_window.is_none());
    }
}
