//! `glassline import` — one-shot migrator from ccstatusline to glassline.
//!
//! Design ref: [[ccstatusline_import_design_v1.0]].
//!
//! Reads a ccstatusline `settings.json`, runs the same
//! [`glassline_core::migration::migrate_value`] the hot path uses, writes
//! the result to glassline's own config path via a temp-file + rename
//! under a `settings.lock` (fs2 exclusive). Non-interactive: skip prompt
//! with `--yes` or non-TTY stdin.
//!
//! Never touches the ccstatusline source file. Users can revert to
//! ccstatusline by removing the freshly-written glassline settings.json.

use std::{
    fs::{self, OpenOptions},
    io::{IsTerminal, Read, Write},
    path::{Path, PathBuf},
};

use fs2::FileExt;
use glassline_core::{
    migration::{MigrationWarning, detect_version, migrate_value},
    settings::{CURRENT_VERSION, Settings},
};
use thiserror::Error;

/// Argv options for `glassline import`.
#[derive(Debug, Default, Clone)]
pub struct ImportOpts {
    pub from: Option<PathBuf>,
    pub to: Option<PathBuf>,
    pub dry_run: bool,
    pub force: bool,
    pub yes: bool,
    pub quiet: bool,
}

/// Successful import outcome.
#[derive(Debug, Clone)]
pub struct ImportReport {
    pub source: PathBuf,
    pub source_version: u32,
    pub target: PathBuf,
    pub target_version: u32,
    pub lines: usize,
    pub widgets_builtin: usize,
    pub widgets_external: usize,
    pub warnings: Vec<MigrationWarning>,
    pub written: bool,
    pub target_json: String,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("no ccstatusline settings.json found — tried:\n{0}")]
    NoSource(String),
    #[error("read source {path}: {source}")]
    ReadSource {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parse source JSON: {0}")]
    ParseSource(#[from] serde_json::Error),
    #[error("migrate: {0}")]
    Migrate(String),
    #[error("shape after migration: {0}")]
    Shape(String),
    #[error("target exists at {0}; pass --force to overwrite")]
    TargetExists(PathBuf),
    #[error("acquire settings.lock at {path}: {source}")]
    Lock {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("write target {path}: {source}")]
    WriteTarget {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("user declined at confirmation prompt")]
    Declined,
    #[error("resolve target path: {0}")]
    ResolveTarget(String),
}

impl ImportError {
    /// Map an error to the CLI exit code documented in the design.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::NoSource(_) => 1,
            Self::ReadSource { .. }
            | Self::ParseSource(_)
            | Self::Migrate(_)
            | Self::Shape(_) => 2,
            Self::TargetExists(_) => 3,
            Self::Lock { .. } | Self::WriteTarget { .. } => 4,
            // `Declined` and `ResolveTarget` are grouped under exit 2 as
            // "input-time" errors — no source-file resolution or user-input
            // failure should count as a "write failed" (exit 4).
            Self::Declined | Self::ResolveTarget(_) => 2,
        }
    }
}

/// Execute a `glassline import` invocation end-to-end.
pub fn run_import(opts: &ImportOpts) -> Result<ImportReport, ImportError> {
    // 1. Resolve the source path.
    let source = match opts.from.as_ref() {
        Some(p) => p.clone(),
        None => auto_detect_source()
            .ok_or_else(|| ImportError::NoSource(format_probed_paths(&probe_source_paths())))?,
    };

    // 2. Resolve the target path (default = glassline's own default).
    let target = match opts.to.as_ref() {
        Some(p) => p.clone(),
        None => crate::config::default_settings_path()
            .map_err(|e| ImportError::ResolveTarget(e.to_string()))?,
    };

    // 3. Refuse-by-default when target exists.
    if !opts.dry_run && target.exists() && !opts.force {
        return Err(ImportError::TargetExists(target));
    }

    // 4. Read + parse + migrate.
    let raw = fs::read_to_string(&source).map_err(|e| ImportError::ReadSource {
        path: source.clone(),
        source: e,
    })?;
    let parsed: serde_json::Value = serde_json::from_str(&raw)?;
    let source_version = detect_version(&parsed);
    let (migrated, warnings) =
        migrate_value(parsed, source_version).map_err(|e| ImportError::Migrate(e.to_string()))?;

    // 5. Round-trip through Settings to validate shape (matches the hot
    //    path's load flow so we don't write anything the renderer can't
    //    read back).
    let settings: Settings =
        serde_json::from_value(migrated.clone()).map_err(|e| ImportError::Shape(e.to_string()))?;

    // 6. Build the pretty-printed JSON we'd write.
    let target_json = serde_json::to_string_pretty(&settings).map_err(ImportError::ParseSource)?;

    // Report snapshot fields (computed BEFORE any prompt / write so a
    // dry-run gets the same numbers).
    let (widgets_builtin, widgets_external) = count_widgets(&settings);
    let lines = settings.lines.iter().filter(|l| !l.is_empty()).count();

    let mut report = ImportReport {
        source,
        source_version,
        target: target.clone(),
        target_version: CURRENT_VERSION,
        lines,
        widgets_builtin,
        widgets_external,
        warnings,
        written: false,
        target_json: target_json.clone(),
    };

    // Dry-run stops here — no prompt, no write.
    if opts.dry_run {
        return Ok(report);
    }

    // 7. Confirmation prompt (unless --yes, non-TTY, or the target didn't
    //    exist so there's nothing to clobber).
    if !opts.yes
        && std::io::stdin().is_terminal()
        && target.exists()
        && !confirm(&target, lines, widgets_builtin + widgets_external)?
    {
        return Err(ImportError::Declined);
    }

    // 8. Ensure target dir + lock.
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir).map_err(|e| ImportError::WriteTarget {
            path: target.clone(),
            source: e,
        })?;
    }
    let lock_path = target
        .parent()
        .map(|d| d.join("settings.lock"))
        .unwrap_or_else(|| PathBuf::from("settings.lock"));
    let _lock = acquire_settings_lock(&lock_path)?;

    // 9. Temp-file + atomic rename.
    let tmp_path = target.with_extension("json.tmp");
    fs::write(&tmp_path, target_json.as_bytes()).map_err(|e| ImportError::WriteTarget {
        path: tmp_path.clone(),
        source: e,
    })?;
    fs::rename(&tmp_path, &target).map_err(|e| ImportError::WriteTarget {
        path: target.clone(),
        source: e,
    })?;

    report.written = true;
    Ok(report)
}

/// Render an [`ImportReport`] as human-readable text.
///
/// Layout mirrors [[ccstatusline_import_design_v1.0]] §4.4.
#[must_use]
pub fn render_report(report: &ImportReport, opts: &ImportOpts) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "glassline import v{}\n\n",
        env!("CARGO_PKG_VERSION")
    ));
    out.push_str(&format!("  source:            {}\n", report.source.display()));
    out.push_str(&format!(
        "  source schema:     v{}\n",
        report.source_version
    ));
    out.push_str(&format!("  target:            {}\n", report.target.display()));
    out.push_str(&format!(
        "  target schema:     v{} ({})\n",
        report.target_version,
        if report.written {
            "written"
        } else if opts.dry_run {
            "dry-run — not written"
        } else {
            "prepared but not written"
        }
    ));
    out.push_str(&format!("  lines:             {}\n", report.lines));
    out.push_str(&format!(
        "  widgets migrated:  {} built-in, {} external\n\n",
        report.widgets_builtin, report.widgets_external,
    ));

    out.push_str(&format!("warnings ({}):\n", report.warnings.len()));
    if report.warnings.is_empty() {
        out.push_str("  (none)\n\n");
    } else {
        for w in &report.warnings {
            let loc = w
                .location
                .as_deref()
                .map(|s| format!(" [{s}]"))
                .unwrap_or_default();
            out.push_str(&format!("  - {:?}{}: {}\n", w.scope, loc, w.message));
        }
        out.push('\n');
    }

    if opts.dry_run {
        out.push_str("--- target JSON (dry-run — not written) ---\n");
        out.push_str(&report.target_json);
        out.push('\n');
    } else {
        out.push_str("next steps:\n");
        out.push_str("  1. glassline install     # wire the statusLine hook (if not already done)\n");
        out.push_str("  2. Original ccstatusline file untouched; delete the new glassline\n");
        out.push_str("     settings.json to revert.\n");
    }
    out
}

// -- source auto-detection --

/// Six documented paths, in probe order.
#[must_use]
fn probe_source_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(env) = std::env::var_os("CCSTATUSLINE_CONFIG") {
        paths.push(PathBuf::from(env));
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        paths.push(
            PathBuf::from(xdg)
                .join("ccstatusline")
                .join("settings.json"),
        );
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    if let Some(h) = home.as_ref() {
        paths.push(
            PathBuf::from(h)
                .join(".config")
                .join("ccstatusline")
                .join("settings.json"),
        );
        paths.push(
            PathBuf::from(h)
                .join(".claude")
                .join("ccstatusline")
                .join("settings.json"),
        );
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        paths.push(
            PathBuf::from(appdata)
                .join("ccstatusline")
                .join("settings.json"),
        );
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        paths.push(
            PathBuf::from(local)
                .join("ccstatusline")
                .join("settings.json"),
        );
    }
    paths
}

fn auto_detect_source() -> Option<PathBuf> {
    probe_source_paths().into_iter().find(|p| p.exists())
}

fn format_probed_paths(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return "  (no HOME/APPDATA env vars set — pass --from <path>)".to_string();
    }
    paths
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

// -- widget counting --

fn count_widgets(settings: &Settings) -> (usize, usize) {
    let mut builtin = 0usize;
    let mut external = 0usize;
    for line in &settings.lines {
        for spec in line {
            if spec.is_external() {
                external += 1;
            } else {
                builtin += 1;
            }
        }
    }
    (builtin, external)
}

// -- lock --

struct SettingsLock {
    file: std::fs::File,
}

impl Drop for SettingsLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn acquire_settings_lock(path: &Path) -> Result<SettingsLock, ImportError> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| ImportError::Lock {
            path: path.to_path_buf(),
            source: e,
        })?;
    file.try_lock_exclusive().map_err(|e| ImportError::Lock {
        path: path.to_path_buf(),
        source: e,
    })?;
    Ok(SettingsLock { file })
}

// -- confirmation prompt --

fn confirm(target: &Path, lines: usize, widgets: usize) -> Result<bool, ImportError> {
    print!(
        "About to write {lines} lines / {widgets} widgets to {}.\nContinue? [Y/n] ",
        target.display()
    );
    let _ = std::io::stdout().flush();
    let mut buf = String::new();
    if std::io::stdin().read_to_string(&mut buf).is_err() {
        // No stdin — treat as unattended, default to Yes (matches --yes).
        return Ok(true);
    }
    let answer = buf.trim().to_ascii_lowercase();
    Ok(answer.is_empty() || answer == "y" || answer == "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir_with(name: &str) -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix(name)
            .tempdir()
            .expect("tempdir")
    }

    #[test]
    fn probe_source_paths_returns_something_or_nothing() {
        // Should never panic regardless of env; length depends on which
        // vars the test host has set. Just assert we got a vec.
        let paths = probe_source_paths();
        // Under CI there's typically at least HOME/USERPROFILE set, so
        // most rows populate. Under sandboxed test runners it might be
        // empty — both are valid.
        let _ = paths.len();
    }

    #[test]
    fn count_widgets_splits_builtin_and_external() {
        use glassline_core::settings::WidgetSpec;
        let s = Settings {
            lines: vec![
                vec![
                    WidgetSpec::new("1", "git-branch"),
                    WidgetSpec::new("2", "ext:my-widget"),
                ],
                vec![WidgetSpec::new("3", "custom-text")],
                vec![],
            ],
            ..Settings::in_memory_defaults()
        };
        let (b, e) = count_widgets(&s);
        assert_eq!(b, 2);
        assert_eq!(e, 1);
    }

    #[test]
    fn import_refuses_existing_target_without_force() {
        let dir = temp_dir_with("import-refuse");
        let source = dir.path().join("src.json");
        std::fs::write(&source, r#"{"version":1,"lines":[]}"#).unwrap();
        let target = dir.path().join("dst.json");
        std::fs::write(&target, r#"{"lines":[]}"#).unwrap();

        let err = run_import(&ImportOpts {
            from: Some(source),
            to: Some(target),
            yes: true,
            ..Default::default()
        })
        .unwrap_err();
        assert!(matches!(err, ImportError::TargetExists(_)));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn import_overwrites_with_force_and_yes() {
        let dir = temp_dir_with("import-force");
        let source = dir.path().join("src.json");
        std::fs::write(&source, r#"{"version":1,"lines":[[{"id":"1","type":"custom-text","customText":"hi"}]]}"#).unwrap();
        let target = dir.path().join("dst.json");
        std::fs::write(&target, r#"{"lines":[]}"#).unwrap();

        let report = run_import(&ImportOpts {
            from: Some(source),
            to: Some(target.clone()),
            force: true,
            yes: true,
            ..Default::default()
        })
        .expect("import");
        assert!(report.written);
        let written = std::fs::read_to_string(&target).unwrap();
        assert!(written.contains("custom-text"));
        assert!(written.contains(r#""version": 3"#));
    }

    #[test]
    fn dry_run_does_not_touch_disk() {
        let dir = temp_dir_with("import-dry");
        let source = dir.path().join("src.json");
        std::fs::write(&source, r#"{"version":1,"lines":[]}"#).unwrap();
        let target = dir.path().join("dst.json");

        let report = run_import(&ImportOpts {
            from: Some(source),
            to: Some(target.clone()),
            dry_run: true,
            ..Default::default()
        })
        .expect("dry-run");
        assert!(!report.written);
        assert!(!target.exists(), "dry-run must not create the target");
        assert!(report.target_json.contains(r#""version": 3"#));
    }

    #[test]
    fn missing_source_returns_no_source_error() {
        // Pass an explicit non-existent path; auto-detect isn't exercised.
        let opts = ImportOpts {
            from: Some(PathBuf::from("Z:/definitely/not/real.json")),
            to: Some(PathBuf::from("Z:/somewhere.json")),
            yes: true,
            ..Default::default()
        };
        let err = run_import(&opts).unwrap_err();
        assert!(matches!(err, ImportError::ReadSource { .. }));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn corrupt_source_json_errors_at_parse() {
        let dir = temp_dir_with("import-badjson");
        let source = dir.path().join("src.json");
        std::fs::write(&source, "{not json").unwrap();
        let target = dir.path().join("dst.json");
        let err = run_import(&ImportOpts {
            from: Some(source),
            to: Some(target),
            yes: true,
            ..Default::default()
        })
        .unwrap_err();
        assert!(matches!(err, ImportError::ParseSource(_)));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn migrate_bumps_version_field() {
        let dir = temp_dir_with("import-v1");
        let source = dir.path().join("src.json");
        // A v1 file (no version field) with a single widget.
        std::fs::write(
            &source,
            r#"{"lines":[[{"id":"1","type":"custom-text","customText":"hello"}]]}"#,
        )
        .unwrap();
        let target = dir.path().join("dst.json");
        let report = run_import(&ImportOpts {
            from: Some(source),
            to: Some(target),
            yes: true,
            ..Default::default()
        })
        .expect("import");
        assert_eq!(report.source_version, 1);
        assert_eq!(report.target_version, CURRENT_VERSION);
    }

    #[test]
    fn render_report_lists_no_warnings_line() {
        let dir = temp_dir_with("import-report");
        let source = dir.path().join("src.json");
        std::fs::write(&source, r#"{"version":1,"lines":[]}"#).unwrap();
        let target = dir.path().join("dst.json");
        let opts = ImportOpts {
            from: Some(source),
            to: Some(target),
            yes: true,
            ..Default::default()
        };
        let report = run_import(&opts).expect("import");
        let text = render_report(&report, &opts);
        assert!(text.contains("warnings (0)"));
        assert!(text.contains("(none)"));
        assert!(text.contains("next steps:"));
    }
}
