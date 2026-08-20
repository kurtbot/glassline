//! `glassline install` / `glassline uninstall` — wire glassline into
//! Claude Code's `settings.json` statusline hook.
//!
//! Merges the `statusLine` key into the target `settings.json` without
//! touching any other keys. Writes atomically via a sibling tempfile so an
//! interruption never leaves a truncated `settings.json`.
//!
//! Two scopes:
//! - `user`    → `$CLAUDE_CONFIG_DIR/settings.json`
//!   → `~/.claude/settings.json`
//! - `project` → `<cwd>/.claude/settings.json`

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};

#[cfg(test)]
use serde_json::json;
use thiserror::Error;

/// Which `settings.json` we mutate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
}

/// Options gathered from argv.
#[derive(Debug, Clone)]
pub struct InstallOpts {
    pub scope: Scope,
    /// Write the absolute path of the running exe instead of relying on
    /// `PATH`. Default is bare `glassline` because Claude Code on Windows
    /// invokes statusLine commands through Git Bash — Windows-style paths
    /// with backslashes break bash escaping, and even forward-slash
    /// `C:/…` paths don't resolve under some bash variants (WSL, MSYS
    /// clones). Bare `glassline` on `PATH` sidesteps the whole class of
    /// problems.
    pub absolute_path: bool,
    /// Preview only — never write.
    pub dry_run: bool,
    /// Overwrite an existing `statusLine` entry even if it points elsewhere.
    pub force: bool,
}

impl Default for InstallOpts {
    fn default() -> Self {
        Self {
            scope: Scope::User,
            absolute_path: false,
            dry_run: false,
            force: false,
        }
    }
}

/// Result of a successful `install` / `uninstall`.
#[derive(Debug, Clone)]
pub struct InstallReport {
    pub path: PathBuf,
    pub previous: Option<Value>,
    pub new: Option<Value>,
    pub wrote: bool,
}

/// Public install entry point. Resolves the target `settings.json` from
/// `opts.scope` and delegates to [`install_at`].
pub fn run_install(opts: &InstallOpts) -> Result<InstallReport, InstallError> {
    let path = resolve_settings_path(opts.scope)?;
    install_at(&path, opts)
}

/// Public uninstall entry point. Same resolve → delegate pattern as
/// [`run_install`].
pub fn run_uninstall(opts: &InstallOpts) -> Result<InstallReport, InstallError> {
    let path = resolve_settings_path(opts.scope)?;
    uninstall_at(&path, opts)
}

/// Install into an explicit `settings.json` path. Used both by
/// [`run_install`] (production) and the test harness (bypasses env-var
/// resolution so parallel tests don't race on `CLAUDE_CONFIG_DIR`).
pub fn install_at(path: &Path, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
    let existing = load_settings(path)?;
    let previous = existing.get("statusLine").cloned();

    if let Some(prev) = &previous
        && !opts.force
    {
        if is_glassline_entry(prev) {
            return Ok(InstallReport {
                path: path.to_path_buf(),
                previous: previous.clone(),
                new: previous,
                wrote: false,
            });
        }
        return Err(InstallError::AlreadyConfigured {
            path: path.to_path_buf(),
            existing: prev.clone(),
        });
    }

    let command = if opts.absolute_path {
        current_exe_string()?
    } else {
        "glassline".to_string()
    };
    // Start from the existing statusLine so extra keys the user tuned
    // (e.g. `padding`, `refreshInterval`) survive an overwrite. If the
    // existing entry isn't an object (or missing), start empty.
    let mut new_map = previous
        .as_ref()
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    new_map.insert("type".into(), Value::String("command".into()));
    new_map.insert("command".into(), Value::String(command));
    let new_entry = Value::Object(new_map);
    let mut updated = existing;
    if !updated.is_object() {
        updated = Value::Object(Map::new());
    }
    updated
        .as_object_mut()
        .expect("json object")
        .insert("statusLine".into(), new_entry.clone());

    let wrote = if opts.dry_run {
        false
    } else {
        write_settings_atomic(path, &updated)?;
        true
    };

    Ok(InstallReport {
        path: path.to_path_buf(),
        previous,
        new: Some(new_entry),
        wrote,
    })
}

/// Uninstall from an explicit `settings.json` path. Symmetric with
/// [`install_at`].
pub fn uninstall_at(path: &Path, opts: &InstallOpts) -> Result<InstallReport, InstallError> {
    let mut existing = load_settings(path)?;
    let previous = existing.get("statusLine").cloned();

    let Some(prev) = &previous else {
        return Ok(InstallReport {
            path: path.to_path_buf(),
            previous: None,
            new: None,
            wrote: false,
        });
    };

    if !opts.force && !is_glassline_entry(prev) {
        return Err(InstallError::NotGlassline {
            path: path.to_path_buf(),
            existing: prev.clone(),
        });
    }

    if let Value::Object(ref mut map) = existing {
        map.remove("statusLine");
    }

    let wrote = if opts.dry_run {
        false
    } else {
        write_settings_atomic(path, &existing)?;
        true
    };

    Ok(InstallReport {
        path: path.to_path_buf(),
        previous,
        new: None,
        wrote,
    })
}

fn is_glassline_entry(value: &Value) -> bool {
    let Some(cmd) = value.get("command").and_then(Value::as_str) else {
        return false;
    };
    let leaf = Path::new(cmd)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(cmd);
    leaf.eq_ignore_ascii_case("glassline")
}

fn resolve_settings_path(scope: Scope) -> Result<PathBuf, InstallError> {
    let dir = match scope {
        Scope::User => user_claude_dir()?,
        Scope::Project => std::env::current_dir()
            .map_err(|e| InstallError::Io {
                path: PathBuf::from("."),
                source: e,
            })?
            .join(".claude"),
    };
    if let Err(e) = fs::create_dir_all(&dir) {
        return Err(InstallError::Io {
            path: dir.clone(),
            source: e,
        });
    }
    Ok(dir.join("settings.json"))
}

fn user_claude_dir() -> Result<PathBuf, InstallError> {
    if let Ok(override_dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or(InstallError::NoHome)?;
    Ok(PathBuf::from(home).join(".claude"))
}

fn load_settings(path: &Path) -> Result<Value, InstallError> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = fs::read_to_string(path).map_err(|e| InstallError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&raw).map_err(|e| InstallError::ParseExisting {
        path: path.to_path_buf(),
        source: e,
    })
}

fn write_settings_atomic(path: &Path, value: &Value) -> Result<(), InstallError> {
    let parent = path.parent().ok_or_else(|| InstallError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other("settings.json has no parent directory"),
    })?;
    fs::create_dir_all(parent).map_err(|e| InstallError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;
    let temp = parent.join(format!(
        ".{}.glassline-tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("settings.json")
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

fn current_exe_string() -> Result<String, InstallError> {
    let exe = std::env::current_exe().map_err(|_| InstallError::NoExePath)?;
    Ok(normalize_for_bash(exe.to_string_lossy().as_ref()))
}

/// Claude Code on Windows runs statusLine commands through Git Bash when
/// available. Bash treats `\` as an escape character, so a native Windows
/// path (`C:\Users\...`) collapses to `C:Users...` and the exe isn't found —
/// silent failure, blank statusline. Rewrite backslashes to forward
/// slashes; both Git Bash and PowerShell accept forward-slash paths on
/// Windows.
#[must_use]
pub fn normalize_for_bash(raw: &str) -> String {
    if !cfg!(windows) {
        return raw.to_string();
    }
    raw.replace('\\', "/")
}

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("cannot locate ~/.claude — set HOME or USERPROFILE or CLAUDE_CONFIG_DIR")]
    NoHome,
    #[error("cannot resolve glassline's own path via current_exe()")]
    NoExePath,
    #[error(
        "{path} already has a statusLine entry pointing elsewhere:\n  {existing}\n\nRe-run with --force to overwrite."
    )]
    AlreadyConfigured { path: PathBuf, existing: Value },
    #[error(
        "{path} statusLine does not look like glassline:\n  {existing}\n\nRe-run with --force to remove anyway."
    )]
    NotGlassline { path: PathBuf, existing: Value },
    #[error("io error on {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("existing settings.json at {path} isn't valid JSON: {source}")]
    ParseExisting {
        path: PathBuf,
        source: serde_json::Error,
    },
}

/// Format an [`InstallReport`] into a terse, user-facing summary.
#[must_use]
pub fn render_report(report: &InstallReport, action: &str) -> String {
    let mut out = String::new();
    let header = if report.wrote {
        format!("{action}: OK")
    } else if report.previous.is_some() && report.new == report.previous {
        format!("{action}: already up to date")
    } else {
        format!("{action}: dry-run (no changes written)")
    };
    out.push_str(&format!("{header}\n  file: {}\n", report.path.display()));
    if let Some(prev) = &report.previous {
        out.push_str(&format!(
            "  before: {}\n",
            serde_json::to_string(prev).unwrap_or_default()
        ));
    }
    if let Some(new) = &report.new {
        out.push_str(&format!(
            "  after:  {}\n",
            serde_json::to_string(new).unwrap_or_default()
        ));
    } else {
        out.push_str("  after:  (statusLine removed)\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny inline temp-dir helper; avoids pulling `tempfile` into
    /// production dependencies just for tests.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "glassline-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0),
            ));
            let _ = std::fs::remove_dir_all(&base);
            std::fs::create_dir_all(&base).unwrap();
            Self(base)
        }
        fn settings(&self) -> PathBuf {
            self.0.join("settings.json")
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Test helper — `use_path` here retains the pre-refactor semantics
    /// (`true` = bare `glassline` on `PATH`) so existing test bodies keep
    /// reading the same way.
    fn opts(force: bool, use_path: bool, dry_run: bool) -> InstallOpts {
        InstallOpts {
            scope: Scope::User,
            absolute_path: !use_path,
            dry_run,
            force,
        }
    }

    #[test]
    fn install_creates_settings_when_absent() {
        let td = TempDir::new("mkfile");
        let report = install_at(&td.settings(), &opts(false, true, false)).unwrap();
        assert!(report.wrote);
        assert!(td.settings().exists());
        let content = fs::read_to_string(td.settings()).unwrap();
        assert!(content.contains("\"statusLine\""));
        assert!(content.contains("\"glassline\""));
    }

    #[test]
    fn install_preserves_other_keys() {
        let td = TempDir::new("preserve");
        fs::write(td.settings(), r#"{"model":"opus","theme":"dark"}"#).unwrap();
        let report = install_at(&td.settings(), &opts(false, true, false)).unwrap();
        assert!(report.wrote);
        let content = fs::read_to_string(td.settings()).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.get("model").and_then(Value::as_str), Some("opus"));
        assert_eq!(parsed.get("theme").and_then(Value::as_str), Some("dark"));
        assert!(parsed.get("statusLine").is_some());
    }

    #[test]
    fn install_refuses_to_clobber_existing_hook() {
        let td = TempDir::new("existing");
        fs::write(
            td.settings(),
            r#"{"statusLine":{"type":"command","command":"other-tool"}}"#,
        )
        .unwrap();
        let err = install_at(&td.settings(), &opts(false, true, false)).unwrap_err();
        assert!(matches!(err, InstallError::AlreadyConfigured { .. }));
    }

    #[test]
    fn install_force_overwrites_foreign_hook() {
        let td = TempDir::new("force");
        fs::write(
            td.settings(),
            r#"{"statusLine":{"type":"command","command":"other-tool"}}"#,
        )
        .unwrap();
        let report = install_at(&td.settings(), &opts(true, true, false)).unwrap();
        assert!(report.wrote);
        let content = fs::read_to_string(td.settings()).unwrap();
        assert!(content.contains("\"glassline\""));
    }

    #[test]
    fn install_is_noop_when_already_glassline() {
        let td = TempDir::new("noop");
        let _first = install_at(&td.settings(), &opts(false, true, false)).unwrap();
        let second = install_at(&td.settings(), &opts(false, true, false)).unwrap();
        assert!(!second.wrote, "second install should be a noop");
    }

    #[test]
    fn uninstall_removes_glassline_hook() {
        let td = TempDir::new("uninstall");
        install_at(&td.settings(), &opts(false, true, false)).unwrap();
        let report = uninstall_at(&td.settings(), &opts(false, true, false)).unwrap();
        assert!(report.wrote);
        let content = fs::read_to_string(td.settings()).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert!(parsed.get("statusLine").is_none());
    }

    #[test]
    fn uninstall_refuses_foreign_hook_without_force() {
        let td = TempDir::new("uninstall-foreign");
        fs::write(
            td.settings(),
            r#"{"statusLine":{"type":"command","command":"other-tool"}}"#,
        )
        .unwrap();
        let err = uninstall_at(&td.settings(), &opts(false, true, false)).unwrap_err();
        assert!(matches!(err, InstallError::NotGlassline { .. }));
    }

    #[test]
    fn is_glassline_entry_matches_absolute_paths() {
        let v = json!({"type":"command","command":"/home/u/.cargo/bin/glassline"});
        assert!(is_glassline_entry(&v));
        let win = json!({"type":"command","command":"C:\\Users\\x\\.cargo\\bin\\glassline.exe"});
        assert!(is_glassline_entry(&win));
        let other = json!({"type":"command","command":"/opt/other/tool"});
        assert!(!is_glassline_entry(&other));
    }

    #[test]
    fn dry_run_does_not_write() {
        let td = TempDir::new("dryrun");
        let report = install_at(&td.settings(), &opts(false, true, true)).unwrap();
        assert!(!report.wrote);
        assert!(!td.settings().exists());
    }

    #[cfg(windows)]
    #[test]
    fn install_uses_forward_slashes_on_windows() {
        let td = TempDir::new("winslash");
        let report = install_at(&td.settings(), &opts(false, false, false)).unwrap();
        let cmd = report
            .new
            .as_ref()
            .and_then(|v| v.get("command"))
            .and_then(Value::as_str)
            .expect("command written");
        assert!(!cmd.contains('\\'), "expected forward slashes, got {cmd:?}");
        assert!(cmd.contains('/'), "expected path separators, got {cmd:?}");
    }

    #[test]
    fn normalize_for_bash_leaves_forward_slashes_alone() {
        assert_eq!(normalize_for_bash("/usr/local/bin/x"), "/usr/local/bin/x");
    }

    #[test]
    fn install_force_preserves_extra_keys() {
        let td = TempDir::new("preserve-extras");
        fs::write(
            td.settings(),
            r#"{"statusLine":{"type":"command","command":"old-tool","padding":0,"refreshInterval":10}}"#,
        )
        .unwrap();
        let report = install_at(&td.settings(), &opts(true, true, false)).unwrap();
        assert!(report.wrote);
        let content = fs::read_to_string(td.settings()).unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        let status = parsed.get("statusLine").unwrap();
        assert_eq!(
            status.get("command").and_then(Value::as_str),
            Some("glassline")
        );
        assert_eq!(status.get("padding").and_then(Value::as_i64), Some(0));
        assert_eq!(
            status.get("refreshInterval").and_then(Value::as_i64),
            Some(10)
        );
    }

    #[test]
    fn uninstall_missing_hook_is_noop() {
        let td = TempDir::new("uninstall-none");
        fs::write(td.settings(), r#"{"model":"opus"}"#).unwrap();
        let report = uninstall_at(&td.settings(), &opts(false, true, false)).unwrap();
        assert!(!report.wrote);
    }
}
