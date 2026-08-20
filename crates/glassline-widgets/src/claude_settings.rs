//! Layered reader for Claude Code's `~/.claude/settings.json` stack.
//!
//! Ports `sirmalloc/ccstatusline/src/utils/claude-settings.ts` helpers
//! (`getSandboxConfig`, `getVoiceConfig`, `getRemoteControlStatus`,
//! `resolveClaudeConfigCwd`).
//!
//! Layered read order matches upstream, highest priority first:
//! 1. `<cwd>/.claude/settings.local.json`     (per-project runtime override)
//! 2. `<cwd>/.claude/settings.json`           (per-project committed config)
//! 3. `<userDir>/.claude/settings.local.json` (user runtime override)
//! 4. `<userDir>/.claude/settings.json`       (user committed config)
//!
//! `userDir` = `$CLAUDE_CONFIG_DIR` if set (must be a valid directory),
//! else `$HOME` on Unix or `%USERPROFILE%` on Windows.
//!
//! Return semantics for [`read_layered_bool`]:
//! - `None` — no candidate file exists (Claude Code never initialised).
//!   Widgets that hide themselves in this case return `Vec::new()`.
//! - `Some(false)` — files exist but no explicit override.
//! - `Some(true)` / `Some(false)` — first explicit override wins.

use std::path::{Path, PathBuf};

use glassline_core::render_context::RenderContext;
use serde_json::Value;

/// Resolve the user's `.claude` directory. `$CLAUDE_CONFIG_DIR` wins if
/// it points at an existing dir; falls back to `$HOME`/`$USERPROFILE`.
#[must_use]
pub fn claude_user_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".claude"))
}

/// Resolve the path to `~/.claude.json` — Claude Code's top-level user
/// config, distinct from the layered `.claude/settings.json` stack. Holds
/// account metadata like `oauthAccount.emailAddress` (read by
/// `claude-account-email`).
///
/// `$CLAUDE_CONFIG_DIR` overrides the default location if set to a valid
/// directory; the file itself is always named `.claude.json`.
#[must_use]
pub fn claude_json_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p.join(".claude.json"));
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(home).join(".claude.json"))
}

/// Read `oauthAccount.emailAddress` from `~/.claude.json`. Returns `None`
/// on missing file, malformed JSON, missing field, or empty string.
#[must_use]
pub fn read_oauth_account_email() -> Option<String> {
    let path = claude_json_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let email = v
        .get("oauthAccount")?
        .get("emailAddress")?
        .as_str()?
        .trim();
    if email.is_empty() {
        return None;
    }
    Some(email.to_string())
}

/// Pick the working directory to root project-local settings reads in.
///
/// Priority (matches upstream `resolveClaudeConfigCwd`):
/// 1. `data.workspace.project_dir`
/// 2. `data.cwd`
/// 3. `data.workspace.current_dir`
///
/// Whitespace-only strings are treated as absent.
#[must_use]
pub fn resolve_claude_config_cwd(ctx: &RenderContext) -> Option<PathBuf> {
    let data = ctx.data.as_ref()?;
    let candidates = [
        data.workspace
            .as_ref()
            .and_then(|w| w.project_dir.as_deref()),
        data.cwd.as_deref(),
        data.workspace
            .as_ref()
            .and_then(|w| w.current_dir.as_deref()),
    ];
    for c in candidates {
        if let Some(s) = c
            && !s.trim().is_empty()
        {
            return Some(PathBuf::from(s));
        }
    }
    None
}

/// Four candidate settings paths, highest priority first, deduplicated.
#[must_use]
pub fn layered_settings_paths(cwd: &Path) -> Vec<PathBuf> {
    let project_claude = cwd.join(".claude");
    let mut paths = vec![
        project_claude.join("settings.local.json"),
        project_claude.join("settings.json"),
    ];
    if let Some(user_dir) = claude_user_dir() {
        paths.push(user_dir.join("settings.local.json"));
        paths.push(user_dir.join("settings.json"));
    }
    // Dedup while preserving priority order (project settings may equal
    // user settings when `cwd` is exactly the user's `.claude`).
    let mut seen: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for p in paths {
        if !seen.iter().any(|q| q == &p) {
            seen.push(p);
        }
    }
    seen
}

/// Walk a JSON object down a dotted-key path, returning the terminal
/// [`Value`] if present.
fn walk<'a>(root: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let mut cur = root;
    for key in keys {
        cur = cur.get(key)?;
    }
    Some(cur)
}

/// Read a nested boolean setting from the layered stack.
///
/// See module docs for return semantics.
#[must_use]
pub fn read_layered_bool(cwd: &Path, keys: &[&str]) -> Option<LayeredBool> {
    let mut any_existed = false;
    for candidate in layered_settings_paths(cwd) {
        match std::fs::read_to_string(&candidate) {
            Ok(raw) => {
                any_existed = true;
                if let Ok(v) = serde_json::from_str::<Value>(&raw)
                    && let Some(b) = walk(&v, keys).and_then(Value::as_bool)
                {
                    return Some(LayeredBool::Explicit(b));
                }
                // File parsed but no override — fall through to next layer.
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                // Any other I/O error is treated like the file being
                // there but unreadable — matches upstream's coarse
                // treatment.
                any_existed = true;
                continue;
            }
        }
    }
    if any_existed {
        Some(LayeredBool::DefaultFalse)
    } else {
        None
    }
}

/// Outcome of a layered-bool read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayeredBool {
    /// An explicit override was found — the value is authoritative.
    Explicit(bool),
    /// Files exist but no explicit override — Claude Code's default is
    /// `false` for both `sandbox.enabled` and `voice.enabled`.
    DefaultFalse,
}

impl LayeredBool {
    /// Collapse to a plain bool via the "default is false" rule.
    #[must_use]
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Explicit(true))
    }
}

#[cfg(test)]
#[allow(unsafe_code)]
// std::env::set_var / remove_var are `unsafe` in Rust 2024; required
// here because these tests exercise `$CLAUDE_CONFIG_DIR`-driven behaviour
// in `claude_user_dir()`. All env mutations serialise on TEST_ENV_LOCK.
mod tests {
    use super::*;
    use crate::common::TEST_ENV_LOCK;

    fn tmp(name: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(&format!("glassline-claude-settings-{name}-"))
            .tempdir()
            .expect("tempdir")
    }

    fn set_user_dir(dir: &Path) {
        unsafe { std::env::set_var("CLAUDE_CONFIG_DIR", dir) };
    }
    fn unset_user_dir() {
        unsafe { std::env::remove_var("CLAUDE_CONFIG_DIR") };
    }

    #[test]
    fn returns_none_when_no_files_exist() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("none");
        let cwd = td.path().join("workspace");
        std::fs::create_dir_all(&cwd).unwrap();
        let user_dir = td.path().join("home-claude");
        std::fs::create_dir_all(&user_dir).unwrap();
        set_user_dir(&user_dir);

        assert_eq!(
            read_layered_bool(&cwd, &["sandbox", "enabled"]),
            None
        );
        unset_user_dir();
    }

    #[test]
    fn returns_default_false_when_files_exist_no_override() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("default");
        let cwd = td.path().join("workspace");
        let proj = cwd.join(".claude");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("settings.json"), r#"{"other":"key"}"#).unwrap();
        let user_dir = td.path().join("home-claude");
        std::fs::create_dir_all(&user_dir).unwrap();
        set_user_dir(&user_dir);

        assert_eq!(
            read_layered_bool(&cwd, &["sandbox", "enabled"]),
            Some(LayeredBool::DefaultFalse)
        );
        unset_user_dir();
    }

    #[test]
    fn project_local_wins_over_project_and_user() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("priority");
        let cwd = td.path().join("workspace");
        let proj = cwd.join(".claude");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(
            proj.join("settings.local.json"),
            r#"{"sandbox":{"enabled":true}}"#,
        )
        .unwrap();
        std::fs::write(
            proj.join("settings.json"),
            r#"{"sandbox":{"enabled":false}}"#,
        )
        .unwrap();

        let user_dir = td.path().join("home-claude");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(
            user_dir.join("settings.json"),
            r#"{"sandbox":{"enabled":false}}"#,
        )
        .unwrap();
        set_user_dir(&user_dir);

        assert_eq!(
            read_layered_bool(&cwd, &["sandbox", "enabled"]),
            Some(LayeredBool::Explicit(true))
        );
        unset_user_dir();
    }

    #[test]
    fn user_layer_used_when_no_project_layer() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("userlayer");
        let cwd = td.path().join("workspace");
        std::fs::create_dir_all(&cwd).unwrap();
        let user_dir = td.path().join("home-claude");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(
            user_dir.join("settings.json"),
            r#"{"sandbox":{"enabled":true}}"#,
        )
        .unwrap();
        set_user_dir(&user_dir);

        assert_eq!(
            read_layered_bool(&cwd, &["sandbox", "enabled"]),
            Some(LayeredBool::Explicit(true))
        );
        unset_user_dir();
    }

    #[test]
    fn malformed_json_falls_through_to_next_layer() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("malformed");
        let cwd = td.path().join("workspace");
        let proj = cwd.join(".claude");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(proj.join("settings.local.json"), "{not json").unwrap();
        let user_dir = td.path().join("home-claude");
        std::fs::create_dir_all(&user_dir).unwrap();
        std::fs::write(
            user_dir.join("settings.json"),
            r#"{"sandbox":{"enabled":true}}"#,
        )
        .unwrap();
        set_user_dir(&user_dir);

        assert_eq!(
            read_layered_bool(&cwd, &["sandbox", "enabled"]),
            Some(LayeredBool::Explicit(true))
        );
        unset_user_dir();
    }

    #[test]
    fn nested_key_path_walks_correctly() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("nested");
        let cwd = td.path().join("workspace");
        let proj = cwd.join(".claude");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::write(
            proj.join("settings.json"),
            r#"{"voice":{"enabled":true},"sandbox":{"enabled":false}}"#,
        )
        .unwrap();
        let user_dir = td.path().join("home-claude");
        std::fs::create_dir_all(&user_dir).unwrap();
        set_user_dir(&user_dir);

        assert_eq!(
            read_layered_bool(&cwd, &["voice", "enabled"]),
            Some(LayeredBool::Explicit(true))
        );
        assert_eq!(
            read_layered_bool(&cwd, &["sandbox", "enabled"]),
            Some(LayeredBool::Explicit(false))
        );
        unset_user_dir();
    }

    #[test]
    fn layered_bool_enabled_collapses_default_to_false() {
        assert!(!LayeredBool::DefaultFalse.enabled());
        assert!(LayeredBool::Explicit(true).enabled());
        assert!(!LayeredBool::Explicit(false).enabled());
    }

    #[test]
    fn oauth_email_read_from_claude_json() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("email");
        // Point CLAUDE_CONFIG_DIR at the temp dir; helper writes
        // .claude.json into that dir when the env var is a valid path.
        std::fs::write(
            td.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"user@example.com"}}"#,
        )
        .unwrap();
        set_user_dir(td.path());

        assert_eq!(
            read_oauth_account_email(),
            Some("user@example.com".to_string())
        );
        unset_user_dir();
    }

    #[test]
    fn oauth_email_none_when_field_missing() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("email-missing");
        std::fs::write(
            td.path().join(".claude.json"),
            r#"{"other":"key"}"#,
        )
        .unwrap();
        set_user_dir(td.path());

        assert_eq!(read_oauth_account_email(), None);
        unset_user_dir();
    }

    #[test]
    fn oauth_email_none_when_empty_string() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("email-empty");
        std::fs::write(
            td.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"   "}}"#,
        )
        .unwrap();
        set_user_dir(td.path());

        assert_eq!(read_oauth_account_email(), None);
        unset_user_dir();
    }

    #[test]
    fn oauth_email_none_when_malformed_json() {
        let _g = TEST_ENV_LOCK.lock().unwrap();
        let td = tmp("email-bad");
        std::fs::write(td.path().join(".claude.json"), "{not json").unwrap();
        set_user_dir(td.path());

        assert_eq!(read_oauth_account_email(), None);
        unset_user_dir();
    }
}
