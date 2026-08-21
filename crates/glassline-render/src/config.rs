//! settings.json loader with path resolution + migration + safer-recovery.
//!
//! Design §4.7. Path precedence:
//!   1. `--config <path>` argv override (highest)
//!   2. `$GLASSLINE_CONFIG`
//!   3. Platform default:
//!      - Linux/Mac: `$XDG_CONFIG_HOME/glassline/settings.json`, else
//!        `~/.config/glassline/settings.json`
//!      - Windows: `%APPDATA%\glassline\settings.json` (via `directories`)
//!
//! Failure modes:
//!   - File absent → return in-memory defaults + `LoadOutcome::FirstRun`.
//!   - File present but not valid JSON → return in-memory defaults +
//!     `LoadOutcome::CorruptFallback { .. }`. The renderer emits a visible
//!     `[glassline: bad settings]` warning line so the user notices without
//!     losing their status line entirely.
//!   - File present + valid JSON + migration succeeds → return parsed
//!     settings + `LoadOutcome::Loaded`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use glassline_core::{
    migration::{MigrationWarning, detect_version, migrate_value},
    settings::Settings,
};
use thiserror::Error;

/// Result of a successful load — the settings the renderer should use plus
/// the story of how we got here.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub settings: Settings,
    pub outcome: LoadOutcome,
    pub path: PathBuf,
    /// Non-fatal notes collected during migration. The hot path discards
    /// these (or logs at DEBUG); `glassline import` surfaces them in its
    /// report; the future TUI wizard renders them in the diff modal.
    pub warnings: Vec<MigrationWarning>,
}

/// What happened when we tried to read `settings.json`.
#[derive(Debug, Clone)]
pub enum LoadOutcome {
    /// The file exists and parsed cleanly (post-migration).
    Loaded,
    /// No settings.json at the resolved path — first run, using defaults.
    FirstRun,
    /// A file was there but couldn't be parsed or migrated. Renderer should
    /// surface a visible warning; we still hand back defaults so the user
    /// isn't left with a blank status bar.
    CorruptFallback { reason: String },
}

/// Load config from an explicit path (`--config`) or from platform defaults.
///
/// Never returns `Err` for a missing file or a parse failure — those become
/// [`LoadOutcome::FirstRun`] / [`LoadOutcome::CorruptFallback`]. The only
/// [`Err`] case is a fundamentally broken environment (no `HOME`,
/// non-object migration output, …) — the caller should treat these as fatal
/// and abort before rendering.
pub fn load(explicit_path: Option<&Path>) -> Result<LoadedConfig, ConfigError> {
    let path = match explicit_path {
        Some(p) => p.to_path_buf(),
        None => default_settings_path()?,
    };

    if !path.exists() {
        return Ok(LoadedConfig {
            settings: Settings::in_memory_defaults(),
            outcome: LoadOutcome::FirstRun,
            path,
            warnings: Vec::new(),
        });
    }

    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            return Ok(LoadedConfig {
                settings: Settings::in_memory_defaults(),
                outcome: LoadOutcome::CorruptFallback {
                    reason: format!("read failed: {e}"),
                },
                path,
                warnings: Vec::new(),
            });
        }
    };
    if raw.trim().is_empty() {
        return Ok(LoadedConfig {
            settings: Settings::in_memory_defaults(),
            outcome: LoadOutcome::FirstRun,
            path,
            warnings: Vec::new(),
        });
    }

    let parsed_value: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return Ok(LoadedConfig {
                settings: Settings::in_memory_defaults(),
                outcome: LoadOutcome::CorruptFallback {
                    reason: format!("json syntax: {e}"),
                },
                path,
                warnings: Vec::new(),
            });
        }
    };

    let source_version = detect_version(&parsed_value);
    let (migrated, warnings) = match migrate_value(parsed_value, source_version) {
        Ok(v) => v,
        Err(e) => {
            return Ok(LoadedConfig {
                settings: Settings::in_memory_defaults(),
                outcome: LoadOutcome::CorruptFallback {
                    reason: format!("migration: {e}"),
                },
                path,
                warnings: Vec::new(),
            });
        }
    };
    let settings: Settings = match serde_json::from_value(migrated) {
        Ok(s) => s,
        Err(e) => {
            return Ok(LoadedConfig {
                settings: Settings::in_memory_defaults(),
                outcome: LoadOutcome::CorruptFallback {
                    reason: format!("shape after migration: {e}"),
                },
                path,
                warnings: Vec::new(),
            });
        }
    };

    Ok(LoadedConfig {
        settings,
        outcome: LoadOutcome::Loaded,
        path,
        warnings,
    })
}

/// Resolve the platform-default `settings.json` path.
pub fn default_settings_path() -> Result<PathBuf, ConfigError> {
    if let Some(override_path) = std::env::var_os("GLASSLINE_CONFIG") {
        return Ok(PathBuf::from(override_path));
    }
    let dir = platform_config_dir()?;
    Ok(dir.join("settings.json"))
}

fn platform_config_dir() -> Result<PathBuf, ConfigError> {
    // Windows: %APPDATA%\glassline
    if cfg!(windows)
        && let Some(appdata) = std::env::var_os("APPDATA")
    {
        return Ok(PathBuf::from(appdata).join("glassline"));
    }
    // XDG on unix.
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(xdg).join("glassline"));
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or(ConfigError::NoHome)?;
    Ok(PathBuf::from(home).join(".config").join("glassline"))
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("cannot locate a config dir — set HOME / USERPROFILE / XDG_CONFIG_HOME / APPDATA")]
    NoHome,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(tag: &str) -> Self {
            let base = std::env::temp_dir().join(format!(
                "glassline-cfg-{}-{}-{}",
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
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn missing_file_is_first_run() {
        let td = TempDir::new("firstrun");
        let path = td.0.join("settings.json");
        let loaded = load(Some(&path)).unwrap();
        assert!(matches!(loaded.outcome, LoadOutcome::FirstRun));
        assert_eq!(loaded.settings, Settings::in_memory_defaults());
    }

    #[test]
    fn valid_settings_load_clean() {
        let td = TempDir::new("loaded");
        let path = td.0.join("settings.json");
        let content = json!({
            "version": 3,
            "lines": [[{"id":"1","type":"custom-text","customText":"hi"}], [], []],
        });
        fs::write(&path, serde_json::to_string_pretty(&content).unwrap()).unwrap();
        let loaded = load(Some(&path)).unwrap();
        assert!(matches!(loaded.outcome, LoadOutcome::Loaded));
        assert_eq!(loaded.settings.version, 3);
        assert_eq!(loaded.settings.lines.len(), 3);
        assert_eq!(loaded.settings.lines[0].len(), 1);
        assert_eq!(loaded.settings.lines[0][0].kind, "custom-text");
    }

    #[test]
    fn legacy_v1_settings_bump_to_current() {
        let td = TempDir::new("v1");
        let path = td.0.join("settings.json");
        // No `version` field → detect_version() returns 1.
        let content = json!({
            "lines": [[{"id":"1","type":"custom-text","customText":"legacy"}]],
        });
        fs::write(&path, content.to_string()).unwrap();
        let loaded = load(Some(&path)).unwrap();
        assert!(matches!(loaded.outcome, LoadOutcome::Loaded));
        assert_eq!(
            loaded.settings.version,
            glassline_core::settings::CURRENT_VERSION
        );
    }

    #[test]
    fn empty_file_is_first_run() {
        let td = TempDir::new("emptyfile");
        let path = td.0.join("settings.json");
        fs::write(&path, "").unwrap();
        let loaded = load(Some(&path)).unwrap();
        assert!(matches!(loaded.outcome, LoadOutcome::FirstRun));
    }

    #[test]
    fn malformed_json_falls_back_with_reason() {
        let td = TempDir::new("badjson");
        let path = td.0.join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let loaded = load(Some(&path)).unwrap();
        assert!(matches!(
            loaded.outcome,
            LoadOutcome::CorruptFallback { .. }
        ));
        assert_eq!(loaded.settings, Settings::in_memory_defaults());
    }

    #[test]
    fn valid_json_wrong_shape_falls_back() {
        let td = TempDir::new("badshape");
        let path = td.0.join("settings.json");
        // `lines` should be `Vec<Vec<WidgetSpec>>`; string here breaks parse.
        fs::write(&path, r#"{"version":3,"lines":"not-an-array"}"#).unwrap();
        let loaded = load(Some(&path)).unwrap();
        assert!(matches!(
            loaded.outcome,
            LoadOutcome::CorruptFallback { .. }
        ));
    }

    #[test]
    fn default_path_resolves_somewhere() {
        // We can't reliably compare against a fixed path across CI matrices;
        // just assert it returns *something* rooted at a plausible dir.
        let path = default_settings_path().unwrap();
        assert!(path.ends_with("settings.json"));
    }
}
