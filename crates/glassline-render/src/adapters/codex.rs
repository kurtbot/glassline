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

use glassline_core::{
    render_context::{RenderContext, TokenMetrics},
    status_json::{ContextWindow, ModelInfo, StatusJson, Workspace},
};

use crate::adapter::CliAdapter;
use crate::install::{InstallError, InstallOpts, InstallReport, Scope};

/// How many lines from the tail of the rollout to parse. Tokens +
/// git summary are emitted per-turn, so the last N=100 lines cover
/// several turns worth of history — more than enough to find the
/// latest value for every field we care about.
const ROLLOUT_TAIL_LINES: usize = 100;

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

                    post_install_hint: None,
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

                post_install_hint: None,
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

            post_install_hint: None,
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

            post_install_hint: None,
        })
    }

    fn unsupported_widgets(&self) -> &'static [&'static str] {
        // Codex has no five-hour block quota concept and its
        // session/usage endpoints differ from Anthropic's. Widgets
        // that key on those fields render as `(unavailable)` when
        // dispatched through this adapter.
        &["block-timer", "block-reset-timer", "session-usage"]
    }

    fn read_context(
        &self,
        stdin: &str,
    ) -> Result<glassline_core::render_context::RenderContext, String> {
        // Two paths:
        //   1. stdin is non-empty and looks like Codex's forward-compat
        //      `statusLine` payload (openai/codex#16921). Parse it as
        //      StatusJSON — its proposed contract mirrors Claude's.
        //   2. stdin is empty (plugin invocation with no piped payload).
        //      Read the latest rollout file under
        //      `$CODEX_HOME/sessions/*.jsonl` and synthesize a
        //      RenderContext from `token_usage_update` /
        //      `thread_status_changed` / `git_summary_update` events.
        if !stdin.trim().is_empty() {
            return parse_forward_compat_statusline(stdin);
        }
        let codex_home =
            resolve_codex_home_readonly().map_err(|e| format!("resolve CODEX_HOME: {e}"))?;
        let rollout = latest_rollout(&codex_home)
            .ok_or_else(|| format!("no rollout file under {}", codex_home.display()))?;
        let summary = parse_rollout(&rollout, ROLLOUT_TAIL_LINES)
            .map_err(|e| format!("parse rollout {}: {e}", rollout.display()))?;
        Ok(rollout_to_context(summary))
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

// --- read_context helpers -------------------------------------------------

/// Read-only sibling of `resolve_codex_home` — doesn't try to create
/// the directory. Used by `read_context` where we only want to look
/// for existing rollout files; refusing to run when the dir is
/// missing is the right behavior.
fn resolve_codex_home_readonly() -> Result<PathBuf, String> {
    if let Some(env) = std::env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(env));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("codex"));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| "no HOME / USERPROFILE / XDG_CONFIG_HOME / CODEX_HOME".to_string())?;
    Ok(PathBuf::from(home).join(".codex"))
}

fn parse_forward_compat_statusline(stdin: &str) -> Result<RenderContext, String> {
    // Codex's proposed `statusLine` payload mirrors Claude's StatusJSON
    // exactly (per openai/codex#16921). Parse via the same type.
    let payload: StatusJson =
        serde_json::from_str(stdin).map_err(|e| format!("parse Codex statusLine payload: {e}"))?;
    let now_ms = now_ms();
    Ok(RenderContext {
        data: Some(payload),
        now_ms,
        ..RenderContext::default()
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Accumulated state we extract from the rollout tail. All fields are
/// Option because a short tail may not include every event type; the
/// mapping to `RenderContext` gracefully skips whatever's None.
#[derive(Debug, Default, Clone)]
pub(crate) struct RolloutSummary {
    pub model_display_name: Option<String>,
    pub model_id: Option<String>,
    pub token_input: Option<u64>,
    pub token_output: Option<u64>,
    pub token_context_length: Option<u64>,
    pub token_cache_read: Option<u64>,
    pub token_cache_creation: Option<u64>,
    pub context_window_size: Option<u64>,
    pub context_used_percentage: Option<f64>,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
}

/// Find the most-recently-modified `.jsonl` file under
/// `<codex_home>/sessions/**/*.jsonl`. Returns None if no rollout
/// exists (the user hasn't started a Codex session yet, or the dir
/// layout differs from what we expect).
///
/// Walks up to 2 levels deep (Codex organizes sessions by date under
/// `sessions/YYYY-MM-DD/<uuid>.jsonl` per its `SESSIONS_SUBDIR`
/// convention). If Codex changes the layout, add another `read_dir`
/// level or switch to a glob crate.
pub(crate) fn latest_rollout(codex_home: &Path) -> Option<PathBuf> {
    let sessions = codex_home.join("sessions");
    if !sessions.is_dir() {
        return None;
    }
    let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    walk_jsonl(&sessions, 2, &mut candidates);
    candidates
        .into_iter()
        .max_by_key(|(mtime, _)| *mtime)
        .map(|(_, p)| p)
}

fn walk_jsonl(dir: &Path, depth_remaining: usize, out: &mut Vec<(std::time::SystemTime, PathBuf)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(md) = entry.metadata() else { continue };
        if md.is_file()
            && path.extension().and_then(|s| s.to_str()) == Some("jsonl")
            && let Ok(mtime) = md.modified()
        {
            out.push((mtime, path));
        } else if md.is_dir() && depth_remaining > 0 {
            walk_jsonl(&path, depth_remaining - 1, out);
        }
    }
}

/// Read the last `max_lines` lines of a rollout file and fold each
/// JSON event into a `RolloutSummary`. Malformed lines are silently
/// skipped — Codex may write partial lines mid-stream, and blowing
/// up on one bad byte would defeat the "best-effort snapshot" goal.
pub(crate) fn parse_rollout(path: &Path, max_lines: usize) -> Result<RolloutSummary, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let lines: Vec<&str> = raw.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    let mut summary = RolloutSummary::default();
    for line in &lines[start..] {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        fold_event(&value, &mut summary);
    }
    Ok(summary)
}

fn fold_event(value: &serde_json::Value, summary: &mut RolloutSummary) {
    // Codex's rollout schema uses a top-level "type" or "event"
    // discriminator plus payload fields. We pattern-match on the
    // discriminator and pull whatever fields are present. Missing
    // fields don't override existing summary values.
    let event_type = value
        .get("type")
        .or_else(|| value.get("event"))
        .and_then(|v| v.as_str());
    match event_type {
        Some("token_usage_update" | "token_usage") => {
            if let Some(usage) = value.get("usage").or_else(|| value.get("payload")) {
                fold_token_usage(usage, summary);
            } else {
                fold_token_usage(value, summary);
            }
        }
        Some("session_configured" | "thread_started" | "session_started") => {
            if let Some(model) = value.get("model") {
                fold_model(model, summary);
            }
            if let Some(id) = value.get("session_id").and_then(|v| v.as_str()) {
                summary.session_id = Some(id.to_string());
            }
            if let Some(id) = value.get("thread_id").and_then(|v| v.as_str()) {
                summary.session_id.get_or_insert(id.to_string());
            }
            if let Some(cwd) = value.get("cwd").and_then(|v| v.as_str()) {
                summary.cwd = Some(cwd.to_string());
            }
        }
        _ => {
            // Some rollout writers put fields at the top level with no
            // discriminator. Best-effort: try to pull known keys.
            if let Some(usage) = value.get("token_usage") {
                fold_token_usage(usage, summary);
            }
            if let Some(model) = value.get("model") {
                fold_model(model, summary);
            }
            if let Some(cwd) = value.get("cwd").and_then(|v| v.as_str()) {
                summary.cwd.get_or_insert(cwd.to_string());
            }
        }
    }
}

fn fold_token_usage(usage: &serde_json::Value, summary: &mut RolloutSummary) {
    if let Some(v) = usage.get("input_tokens").and_then(u64_of) {
        summary.token_input = Some(v);
    }
    if let Some(v) = usage.get("output_tokens").and_then(u64_of) {
        summary.token_output = Some(v);
    }
    if let Some(v) = usage.get("cache_creation_input_tokens").and_then(u64_of) {
        summary.token_cache_creation = Some(v);
    }
    if let Some(v) = usage.get("cache_read_input_tokens").and_then(u64_of) {
        summary.token_cache_read = Some(v);
    }
    if let Some(v) = usage.get("context_length").and_then(u64_of) {
        summary.token_context_length = Some(v);
    }
    if let Some(v) = usage.get("context_window_size").and_then(u64_of) {
        summary.context_window_size = Some(v);
    }
    if let Some(v) = usage.get("used_percentage").and_then(|x| x.as_f64()) {
        summary.context_used_percentage = Some(v);
    }
}

fn fold_model(model: &serde_json::Value, summary: &mut RolloutSummary) {
    if let Some(name) = model.get("display_name").and_then(|v| v.as_str()) {
        summary.model_display_name = Some(name.to_string());
    }
    if let Some(id) = model.get("id").and_then(|v| v.as_str()) {
        summary.model_id = Some(id.to_string());
    }
    if let Some(s) = model.as_str() {
        // Model referenced as a bare string ("gpt-5-codex").
        summary.model_id.get_or_insert(s.to_string());
    }
}

fn u64_of(v: &serde_json::Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_f64().map(|f| f as u64))
}

pub(crate) fn rollout_to_context(summary: RolloutSummary) -> RenderContext {
    let context_window = if summary.token_context_length.is_some()
        || summary.context_window_size.is_some()
        || summary.context_used_percentage.is_some()
    {
        Some(ContextWindow {
            context_window_size: summary.context_window_size.map(|v| v as f64),
            total_input_tokens: summary.token_input.map(|v| v as f64),
            total_output_tokens: summary.token_output.map(|v| v as f64),
            current_usage: None,
            used_percentage: summary.context_used_percentage,
            remaining_percentage: summary.context_used_percentage.map(|p| 100.0 - p),
            usable_percentage: None,
        })
    } else {
        None
    };

    let model = if summary.model_id.is_some() || summary.model_display_name.is_some() {
        Some(ModelInfo::Full {
            id: summary.model_id,
            display_name: summary.model_display_name,
        })
    } else {
        None
    };

    let workspace = summary.cwd.clone().map(|cwd| Workspace {
        current_dir: Some(cwd.clone()),
        project_dir: Some(cwd),
        repo: None,
    });

    let status_json = StatusJson {
        session_id: summary.session_id,
        session_name: None,
        transcript_path: None,
        cwd: summary.cwd,
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

    let token_metrics = if summary.token_input.is_some()
        || summary.token_output.is_some()
        || summary.token_context_length.is_some()
    {
        Some(TokenMetrics {
            input: summary.token_input.unwrap_or(0),
            output: summary.token_output.unwrap_or(0),
            cache_read: summary.token_cache_read.unwrap_or(0),
            cache_creation: summary.token_cache_creation.unwrap_or(0),
            context_length: summary.token_context_length.unwrap_or(0),
        })
    } else {
        None
    };

    RenderContext {
        data: Some(status_json),
        token_metrics,
        now_ms: now_ms(),
        ..RenderContext::default()
    }
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

    // --- rollout parser tests -----------------------------------------

    #[test]
    fn latest_rollout_returns_none_when_no_sessions_dir() {
        let td = TempDir::new("no-sessions");
        assert!(latest_rollout(&td.0).is_none());
    }

    #[test]
    fn latest_rollout_finds_newest_jsonl_across_date_dirs() {
        let td = TempDir::new("multi");
        let sessions = td.0.join("sessions");
        let day1 = sessions.join("2026-08-20");
        let day2 = sessions.join("2026-08-21");
        fs::create_dir_all(&day1).unwrap();
        fs::create_dir_all(&day2).unwrap();
        let older = day1.join("first.jsonl");
        let newer = day2.join("second.jsonl");
        fs::write(&older, "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&newer, "{}").unwrap();
        assert_eq!(latest_rollout(&td.0), Some(newer));
    }

    #[test]
    fn parse_rollout_extracts_token_usage_across_lines() {
        let td = TempDir::new("tokens");
        let path = td.0.join("session.jsonl");
        let lines = [
            r#"{"type":"session_configured","model":{"id":"gpt-5-codex","display_name":"GPT-5 Codex"}}"#,
            r#"{"type":"token_usage_update","usage":{"input_tokens":100,"output_tokens":50,"context_length":150,"context_window_size":200000}}"#,
            r#"{"type":"token_usage_update","usage":{"input_tokens":200,"output_tokens":80,"context_length":280,"context_window_size":200000}}"#,
        ];
        fs::write(&path, lines.join("\n")).unwrap();
        let summary = parse_rollout(&path, 100).unwrap();
        // Latest-wins across events.
        assert_eq!(summary.token_input, Some(200));
        assert_eq!(summary.token_output, Some(80));
        assert_eq!(summary.token_context_length, Some(280));
        assert_eq!(summary.context_window_size, Some(200000));
        assert_eq!(summary.model_id.as_deref(), Some("gpt-5-codex"));
        assert_eq!(summary.model_display_name.as_deref(), Some("GPT-5 Codex"));
    }

    #[test]
    fn parse_rollout_skips_malformed_lines() {
        let td = TempDir::new("malformed");
        let path = td.0.join("session.jsonl");
        let mixed = [
            r#"{"type":"token_usage_update","usage":{"input_tokens":42}}"#,
            "{not-json",
            "",
            r#"{"type":"token_usage_update","usage":{"input_tokens":99}}"#,
        ];
        fs::write(&path, mixed.join("\n")).unwrap();
        let summary = parse_rollout(&path, 100).unwrap();
        assert_eq!(
            summary.token_input,
            Some(99),
            "later valid line wins over noise"
        );
    }

    #[test]
    fn parse_rollout_tail_respects_max_lines() {
        let td = TempDir::new("tail");
        let path = td.0.join("session.jsonl");
        // 5 events; only last 2 should be folded when max_lines=2.
        let events: Vec<String> = (0..5)
            .map(|i| format!(r#"{{"type":"token_usage_update","usage":{{"input_tokens":{i}}}}}"#))
            .collect();
        fs::write(&path, events.join("\n")).unwrap();
        let summary = parse_rollout(&path, 2).unwrap();
        assert_eq!(
            summary.token_input,
            Some(4),
            "only the last event within the tail wins"
        );
    }

    #[test]
    fn rollout_to_context_synthesizes_status_json() {
        let summary = RolloutSummary {
            model_display_name: Some("GPT-5 Codex".to_string()),
            model_id: Some("gpt-5-codex".to_string()),
            token_input: Some(1000),
            token_output: Some(500),
            token_context_length: Some(1500),
            context_window_size: Some(200_000),
            session_id: Some("thr_abc".to_string()),
            cwd: Some("/tmp/proj".to_string()),
            ..Default::default()
        };
        let ctx = rollout_to_context(summary);
        let status = ctx.data.as_ref().expect("StatusJson present");
        assert_eq!(status.session_id.as_deref(), Some("thr_abc"));
        assert_eq!(status.cwd.as_deref(), Some("/tmp/proj"));
        let tokens = ctx.token_metrics.expect("token_metrics present");
        assert_eq!(tokens.input, 1000);
        assert_eq!(tokens.output, 500);
        assert_eq!(tokens.context_length, 1500);
        let cw = status
            .context_window
            .as_ref()
            .expect("context_window present");
        assert_eq!(cw.context_window_size, Some(200_000.0));
    }
}
