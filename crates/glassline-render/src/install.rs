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
    /// If `install` seeded a fresh glassline `settings.json` because
    /// the user didn't have one yet, this holds the path it wrote.
    /// `None` when a config already existed, when `dry_run` was set,
    /// or when seeding failed non-fatally (a warning is printed but
    /// the install still succeeds — the hook is the primary outcome).
    ///
    /// Only ever populated by `run_install`. `install_at` (the pure
    /// tests entry) doesn't seed.
    pub seeded_config: Option<PathBuf>,
    /// A follow-up shell command the user must run for the install
    /// to activate. Populated by adapters whose target CLI requires
    /// an explicit activation step — e.g. Grok's
    /// `grok plugin enable glassline`. `None` for adapters that
    /// self-wire (Claude Code, Codex — the plugin loader picks up
    /// the manifest on next Codex launch without user action).
    pub post_install_hint: Option<String>,
}

/// Public install entry point. Resolves the target `settings.json` from
/// `opts.scope`, delegates to [`install_at`] for the Claude Code hook
/// write, and then (when `dry_run` is off) seeds glassline's own
/// settings.json with the [`templates::power_user`] layout if none
/// exists. Users get a working three-line statusline the moment the
/// Claude Code hook fires, no wizard round-trip required.
pub fn run_install(opts: &InstallOpts) -> Result<InstallReport, InstallError> {
    let path = resolve_settings_path(opts.scope)?;
    let mut report = install_at(&path, opts)?;
    if !opts.dry_run {
        report.seeded_config = seed_default_config_if_missing();
    }
    Ok(report)
}

/// Seed the resolved glassline config path with the `power_user`
/// template if no file exists yet. Returns the path we wrote, or
/// `None` if a config already existed / write failed / resolution
/// failed.
///
/// Failures are intentionally swallowed to stderr — the Claude hook
/// install (the primary outcome) already succeeded by the time this
/// runs, and a seeding failure shouldn't propagate as an error the
/// user sees as "install failed". Losing the seed is a soft downgrade
/// to the pre-seeding behavior (first-run hint shown until the user
/// creates a config manually).
fn seed_default_config_if_missing() -> Option<PathBuf> {
    let path = match crate::config::default_settings_path() {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "glassline install: could not resolve config path to seed default template — {e}\nHook is still wired; run `glassline` in a terminal to create a config."
            );
            return None;
        }
    };
    if path.exists() {
        return None;
    }
    match write_seed_config(&path) {
        Ok(()) => Some(path),
        Err(e) => {
            eprintln!(
                "glassline install: hook wired OK but could not seed default config at {}: {e}\nRun `glassline` in a terminal to create a config manually.",
                path.display()
            );
            None
        }
    }
}

fn write_seed_config(path: &Path) -> Result<(), InstallError> {
    let parent = path.parent().ok_or_else(|| InstallError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::other("config path has no parent directory"),
    })?;
    fs::create_dir_all(parent).map_err(|e| InstallError::Io {
        path: parent.to_path_buf(),
        source: e,
    })?;
    let settings = glassline_core::templates::power_user();
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|e| InstallError::ParseExisting {
        path: path.to_path_buf(),
        source: e,
    })?;
    // Atomic write via tmp + rename — same shape as
    // `write_settings_atomic` but the value type is `Settings`, not
    // `serde_json::Value`.
    let temp = parent.join(format!(
        ".{}.glassline-seed-tmp",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("settings.json")
    ));
    {
        let mut f = fs::File::create(&temp).map_err(|e| InstallError::Io {
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
                seeded_config: None,
                post_install_hint: None,
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
        seeded_config: None,

        post_install_hint: None,
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
            seeded_config: None,

            post_install_hint: None,
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
        seeded_config: None,

        post_install_hint: None,
    })
}

fn is_glassline_entry(value: &Value) -> bool {
    let Some(cmd) = value.get("command").and_then(Value::as_str) else {
        return false;
    };
    // Cross-platform basename extraction: `Path::new` on Linux treats `\`
    // as a literal char, so a Windows-style `C:\...\glassline.exe` stays
    // one segment. Split on BOTH separators before `Path::file_stem`.
    let last_component = cmd.rsplit(['\\', '/']).next().unwrap_or(cmd);
    let leaf = Path::new(last_component)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(last_component);
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

/// Threshold above which the install report nags about `refreshInterval`.
///
/// Claude Code's `refreshInterval` is in SECONDS, minimum `1` (see
/// https://code.claude.com/docs/en/statusline). Animations advance one
/// frame per refresh, so at 5s+ idle refresh, effects like `animate: pulse`
/// and threshold flashing barely move. Users who haven't set any
/// animation metadata don't care; users who have will see the hint.
const REFRESH_INTERVAL_NAG_SECONDS: u64 = 5;

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
    if let Some(seeded) = &report.seeded_config {
        out.push_str(&format!(
            "  seeded: {} (power_user template)\n",
            seeded.display()
        ));
    }
    if let Some(hint) = &report.post_install_hint {
        out.push_str(&format!("\n  Next: {hint}\n"));
    }
    if let Some(secs) = extract_refresh_interval(report.new.as_ref())
        && secs >= REFRESH_INTERVAL_NAG_SECONDS
    {
        out.push_str(&format!(
            "\n  Note: statusLine.refreshInterval is {secs}s — animation effects\n  \
             (animate: pulse, threshold flashing, pulseAbove) advance one frame\n  \
             per refresh. Lower this in {} (minimum 1) for smoother pulses at\n  \
             idle. See https://code.claude.com/docs/en/statusline.\n",
            report.path.display(),
        ));
    }
    out
}

fn extract_refresh_interval(status_line: Option<&Value>) -> Option<u64> {
    status_line?.get("refreshInterval")?.as_u64()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_report_appends_hint_when_refresh_interval_high() {
        // refreshInterval is documented in SECONDS (Claude Code docs,
        // https://code.claude.com/docs/en/statusline).
        let report = InstallReport {
            path: PathBuf::from("/fake/settings.json"),
            previous: None,
            new: Some(json!({
                "type": "command",
                "command": "glassline",
                "refreshInterval": 10
            })),
            wrote: true,
            seeded_config: None,
            post_install_hint: None,
        };
        let text = render_report(&report, "install");
        assert!(
            text.contains("refreshInterval is 10s"),
            "expected cadence hint in report, got: {text}"
        );
    }

    #[test]
    fn render_report_skips_hint_when_refresh_interval_low() {
        let report = InstallReport {
            path: PathBuf::from("/fake/settings.json"),
            previous: None,
            new: Some(json!({
                "type": "command",
                "command": "glassline",
                "refreshInterval": 1
            })),
            wrote: true,
            seeded_config: None,
            post_install_hint: None,
        };
        let text = render_report(&report, "install");
        // The JSON dump includes "refreshInterval": 1 — match on the
        // hint's distinctive prose instead.
        assert!(
            !text.contains("refreshInterval is"),
            "did not expect hint at 1s, got: {text}"
        );
    }

    #[test]
    fn render_report_skips_hint_when_refresh_interval_absent() {
        let report = InstallReport {
            path: PathBuf::from("/fake/settings.json"),
            previous: None,
            new: Some(json!({
                "type": "command",
                "command": "glassline"
            })),
            wrote: true,
            seeded_config: None,
            post_install_hint: None,
        };
        let text = render_report(&report, "install");
        assert!(!text.contains("refreshInterval is"));
    }

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

    #[test]
    fn write_seed_config_creates_power_user_template() {
        let td = TempDir::new("seed-fresh");
        let path = td.0.join("nested").join("settings.json");
        write_seed_config(&path).expect("seed must succeed on a fresh dir");
        assert!(path.exists(), "seed file must be on disk");

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: glassline_core::settings::Settings =
            serde_json::from_str(&content).expect("seed must parse as Settings");
        // Power_user is our shipped template — 3 lines, first widget model.
        assert_eq!(parsed.lines.len(), 3, "power_user has three lines");
        assert_eq!(parsed.lines[0][0].kind, "model");
    }

    #[test]
    fn write_seed_config_creates_missing_parent_dirs() {
        let td = TempDir::new("seed-mkdirs");
        // Nested dir doesn't exist yet — the seed must create it.
        let path = td.0.join("a").join("b").join("c").join("settings.json");
        assert!(!path.parent().unwrap().exists());
        write_seed_config(&path).expect("seed must create parent chain");
        assert!(path.exists());
    }

    #[test]
    fn render_report_shows_seeded_line_when_present() {
        let report = InstallReport {
            path: PathBuf::from("/fake/settings.json"),
            previous: None,
            new: Some(json!({"type": "command", "command": "glassline"})),
            wrote: true,
            seeded_config: Some(PathBuf::from("/fake/.config/glassline/settings.json")),
            post_install_hint: None,
        };
        let text = render_report(&report, "install");
        assert!(
            text.contains("seeded:"),
            "expected 'seeded:' line in report, got: {text}"
        );
        assert!(
            text.contains("power_user template"),
            "expected template name in seeded line, got: {text}"
        );
    }

    #[test]
    fn render_report_hides_seeded_line_when_absent() {
        let report = InstallReport {
            path: PathBuf::from("/fake/settings.json"),
            previous: None,
            new: Some(json!({"type": "command", "command": "glassline"})),
            wrote: true,
            seeded_config: None,
            post_install_hint: None,
        };
        let text = render_report(&report, "install");
        assert!(
            !text.contains("seeded:"),
            "no 'seeded:' line when seeded_config is None, got: {text}"
        );
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
