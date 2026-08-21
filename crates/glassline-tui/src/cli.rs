//! CLI dispatcher for `glassline-tui`.
//!
//! ```text
//! glassline-tui                       # open the interactive editor
//! glassline-tui --config <path>       # override the config path
//! glassline-tui --dry-run [--config P]# validate parseability, exit 0/1
//! glassline-tui --import <path>       # non-interactive migrate + save
//! glassline-tui --export <path>       # dump current scratch to <path>
//! glassline-tui --version             # print version, exit
//! glassline-tui --help                # print usage, exit
//! ```
//!
//! Non-interactive flags never enter alt-screen. They print human-
//! readable status to stdout on success and errors to stderr.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use glassline_core::settings::Settings;
use glassline_render::{
    config::load,
    import::{ImportOpts, run_import},
};
use glassline_tui_dsl::DslApp;

use crate::screens::MainMenu;

/// Parsed CLI shape.
#[derive(Debug, Default)]
struct Args {
    config: Option<PathBuf>,
    action: Action,
}

#[derive(Debug, Default)]
enum Action {
    #[default]
    Tui,
    DryRun,
    Import(PathBuf),
    Export(PathBuf),
    Version,
    Help,
}

/// Entry point invoked by `main`. Returns `Ok(())` on success, `Err`
/// with a human-readable message on failure (main maps that to
/// exit-status 1).
pub fn dispatch<I>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = OsString>,
{
    let parsed = parse(args)?;
    match parsed.action {
        Action::Help => {
            println!("{HELP}");
            Ok(())
        }
        Action::Version => {
            println!("glassline-tui {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Tui => run_tui(parsed.config.as_deref()),
        Action::DryRun => run_dry_run(parsed.config.as_deref()),
        Action::Import(path) => run_import_flag(&path, parsed.config.as_deref()),
        Action::Export(path) => run_export_flag(&path, parsed.config.as_deref()),
    }
}

fn parse<I>(args: I) -> Result<Args, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut it = args.into_iter();
    let mut out = Args::default();
    while let Some(raw) = it.next() {
        let s = raw
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 argument: {}", raw.to_string_lossy()))?
            .to_string();
        match s.as_str() {
            "-h" | "--help" => out.action = Action::Help,
            "-V" | "--version" => out.action = Action::Version,
            "--dry-run" => out.action = Action::DryRun,
            "--config" => {
                let v = it
                    .next()
                    .ok_or_else(|| "--config requires a path".to_string())?;
                out.config = Some(PathBuf::from(v));
            }
            "--import" => {
                let v = it
                    .next()
                    .ok_or_else(|| "--import requires a path".to_string())?;
                out.action = Action::Import(PathBuf::from(v));
            }
            "--export" => {
                let v = it
                    .next()
                    .ok_or_else(|| "--export requires a path".to_string())?;
                out.action = Action::Export(PathBuf::from(v));
            }
            other => return Err(format!("unknown argument: {other:?} (try --help)")),
        }
    }
    Ok(out)
}

fn run_tui(config: Option<&Path>) -> Result<(), String> {
    let (settings, path) = resolve_settings(config);
    let app = DslApp::new(Box::new(MainMenu::new()), settings, path);
    let outcome = app.run().map_err(|e| e.to_string())?;
    println!("Editor exited: {outcome:?}");
    Ok(())
}

fn run_dry_run(config: Option<&Path>) -> Result<(), String> {
    let loaded = load(config).map_err(|e| format!("load: {e}"))?;
    let round_trip = serde_json::to_string_pretty(&loaded.settings)
        .map_err(|e| format!("serialize round-trip: {e}"))?;
    let _reparsed: Settings =
        serde_json::from_str(&round_trip).map_err(|e| format!("reparse round-trip: {e}"))?;
    println!(
        "OK  {}  (version={})",
        loaded.path.display(),
        loaded.settings.version
    );
    println!(
        "    lines={}  widgets_total={}",
        loaded.settings.lines.len(),
        loaded.settings.lines.iter().map(Vec::len).sum::<usize>()
    );
    if !loaded.warnings.is_empty() {
        println!("    migration warnings: {}", loaded.warnings.len());
    }
    Ok(())
}

fn run_import_flag(source: &Path, config: Option<&Path>) -> Result<(), String> {
    let target = config.map(Path::to_path_buf);
    let opts = ImportOpts {
        from: Some(source.to_path_buf()),
        to: target,
        force: true,
        yes: true,
        quiet: false,
        dry_run: false,
    };
    let report = run_import(&opts).map_err(|e| format!("import: {e}"))?;
    println!(
        "Imported {} (v{} -> v{}) -> {}",
        report.source.display(),
        report.source_version,
        report.target_version,
        report.target.display(),
    );
    println!(
        "  lines={}  built-in widgets={}  external widgets={}",
        report.lines, report.widgets_builtin, report.widgets_external,
    );
    if !report.warnings.is_empty() {
        println!("  migration warnings: {}", report.warnings.len());
    }
    Ok(())
}

fn run_export_flag(target: &Path, config: Option<&Path>) -> Result<(), String> {
    let loaded = load(config).map_err(|e| format!("load: {e}"))?;
    if let Some(parent) = target.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let bytes =
        serde_json::to_vec_pretty(&loaded.settings).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(target, bytes).map_err(|e| format!("write {}: {e}", target.display()))?;
    println!("Exported {} -> {}", loaded.path.display(), target.display());
    Ok(())
}

fn resolve_settings(config: Option<&Path>) -> (Settings, PathBuf) {
    match load(config) {
        Ok(loaded) => (loaded.settings, loaded.path),
        Err(_) => (
            Settings::default(),
            PathBuf::from("./glassline-scratch.json"),
        ),
    }
}

const HELP: &str = "\
glassline-tui — interactive layout config editor

USAGE:
  glassline-tui                       Open the interactive editor.
  glassline-tui --config <path>       Override the config path.
  glassline-tui --dry-run [--config P] Validate the config parses cleanly. Exit 0/1.
  glassline-tui --import <path>       Migrate a ccstatusline / glassline settings file
                                      into the resolved config path and save.
  glassline-tui --export <path>       Write the current config to <path>.
  glassline-tui --version             Print version and exit.
  glassline-tui --help                Show this help.

All non-interactive flags exit 0 on success, 1 on failure.";

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_vec(args: &[&str]) -> Result<Args, String> {
        parse(args.iter().map(|s| OsString::from(*s)))
    }

    #[test]
    fn empty_args_default_to_tui() {
        let a = parse_vec(&[]).unwrap();
        assert!(matches!(a.action, Action::Tui));
        assert!(a.config.is_none());
    }

    #[test]
    fn help_and_version_flags() {
        assert!(matches!(
            parse_vec(&["--help"]).unwrap().action,
            Action::Help
        ));
        assert!(matches!(parse_vec(&["-h"]).unwrap().action, Action::Help));
        assert!(matches!(
            parse_vec(&["--version"]).unwrap().action,
            Action::Version
        ));
        assert!(matches!(
            parse_vec(&["-V"]).unwrap().action,
            Action::Version
        ));
    }

    #[test]
    fn dry_run_flag() {
        assert!(matches!(
            parse_vec(&["--dry-run"]).unwrap().action,
            Action::DryRun
        ));
    }

    #[test]
    fn config_override_captures_path() {
        let a = parse_vec(&["--config", "/tmp/foo.json"]).unwrap();
        assert_eq!(
            a.config.as_deref(),
            Some(std::path::Path::new("/tmp/foo.json"))
        );
        assert!(matches!(a.action, Action::Tui));
    }

    #[test]
    fn import_flag_captures_source() {
        let a = parse_vec(&["--import", "src.json"]).unwrap();
        match a.action {
            Action::Import(p) => assert_eq!(p, std::path::Path::new("src.json")),
            other => panic!("expected Import, got {other:?}"),
        }
    }

    #[test]
    fn export_flag_captures_target() {
        let a = parse_vec(&["--export", "out.json"]).unwrap();
        match a.action {
            Action::Export(p) => assert_eq!(p, std::path::Path::new("out.json")),
            other => panic!("expected Export, got {other:?}"),
        }
    }

    #[test]
    fn missing_path_for_flag_errors() {
        assert!(parse_vec(&["--import"]).is_err());
        assert!(parse_vec(&["--export"]).is_err());
        assert!(parse_vec(&["--config"]).is_err());
    }

    #[test]
    fn unknown_flag_errors() {
        assert!(parse_vec(&["--nope"]).is_err());
    }
}
