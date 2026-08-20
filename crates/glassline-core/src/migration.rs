//! Settings-schema migration table (design §4.12).
//!
//! Migrations run on load: each [`Migration`] rewrites a `Value` from its
//! `from` version to `from + 1`. The migration engine is deliberately generic
//! ([`migrate_value`]) — v2→v3 (custom-command → ext:) lands in P2 alongside
//! the [`glassline-ext`] crate.
//!
//! **Warnings.** Every migration step accepts a `&mut Vec<MigrationWarning>`
//! and may push informational or defect-warning entries as it rewrites the
//! value. Warnings are non-fatal: the migration proceeds either way, but the
//! caller can surface them (see `glassline import`'s report per
//! [[ccstatusline_import_design_v1.0]] §4.4) or discard them (the hot-path
//! render loader logs them at DEBUG and moves on).

use serde_json::Value;
use thiserror::Error;

use crate::settings::CURRENT_VERSION;

/// One version-step migration.
pub struct Migration {
    pub from: u32,
    pub to: u32,
    /// Rewrite raw `Value` in place. Called only when the source file is at
    /// [`Self::from`]; expected to produce a value that deserializes as
    /// [`Self::to`]. May push warnings for user-visible details the caller
    /// should surface (e.g. dropped fields, renamed types).
    pub apply: fn(Value, &mut Vec<MigrationWarning>) -> Result<Value, MigrationError>,
}

/// Registry of all shipped migrations, in ascending order by `from`.
///
/// **P2 will insert:** `Migration { from: 2, to: 3, apply: custom_command_to_ext }`.
pub const MIGRATIONS: &[Migration] = &[];

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("settings.json is at version {version} but MIGRATIONS only goes up to {max}")]
    UnknownVersion { version: u32, max: u32 },
    #[error("expected a JSON object at the migration input, got {found}")]
    NotAnObject { found: &'static str },
    #[error("migration v{from}→v{to} failed: {reason}")]
    Failed { from: u32, to: u32, reason: String },
}

/// Non-fatal note produced during migration.
///
/// Emitted for user-visible transformations (e.g. `custom-command` → `ext:*`
/// rewrites, dropped unknown fields, renamed widget IDs) so `glassline import`
/// can surface them in its report and the future TUI wizard can render them
/// in its diff modal. The hot-path render loader logs them at DEBUG and
/// discards them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationWarning {
    pub scope: WarningScope,
    /// Human-readable pointer at where in the settings this warning came from
    /// (e.g. `"line 1, id \"5\", type \"git-branch\""`). `None` when the
    /// warning is global to the file rather than tied to a specific entry.
    pub location: Option<String>,
    pub message: String,
    /// Optional vault design pointer for the report footer / diff modal.
    pub reference: Option<&'static str>,
}

/// Which part of the settings file a [`MigrationWarning`] applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningScope {
    Widget,
    Global,
    Powerline,
    UpdateChecker,
    Installation,
}

/// Run every registered migration whose `from` is ≥ `source_version`.
///
/// Returns the JSON value at [`CURRENT_VERSION`] plus a list of non-fatal
/// warnings collected across every migration step. The caller is responsible
/// for reading the `version` field of the input and passing it here.
pub fn migrate_value(
    mut value: Value,
    source_version: u32,
) -> Result<(Value, Vec<MigrationWarning>), MigrationError> {
    if source_version > CURRENT_VERSION {
        return Err(MigrationError::UnknownVersion {
            version: source_version,
            max: CURRENT_VERSION,
        });
    }
    let mut warnings: Vec<MigrationWarning> = Vec::new();
    let mut current = source_version;
    for migration in MIGRATIONS {
        if migration.from < current {
            continue;
        }
        if migration.from > current {
            // A gap in the migration table means we can't safely upgrade.
            // Skip the rest and report where we stalled.
            return Err(MigrationError::Failed {
                from: current,
                to: current + 1,
                reason: format!("no migration registered from v{current}"),
            });
        }
        value = (migration.apply)(value, &mut warnings)?;
        current = migration.to;
    }
    if current < CURRENT_VERSION {
        // Gap in the migration table — no schema change registered between
        // `current` and `CURRENT_VERSION`. Bump the version field in-place
        // and carry on. This is the forgiving default; real migrations
        // insert themselves into `MIGRATIONS` and take over when they need
        // to rewrite fields.
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "version".into(),
                serde_json::Value::from(u64::from(CURRENT_VERSION)),
            );
        }
    }
    Ok((value, warnings))
}

/// Convenience for detecting the `version` field on a raw settings blob.
///
/// Returns `1` when the field is absent (matches TS `SettingsSchema_v1`).
#[must_use]
pub fn detect_version(value: &Value) -> u32 {
    value
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detect_missing_version_defaults_to_1() {
        assert_eq!(detect_version(&json!({})), 1);
    }

    #[test]
    fn detect_explicit_version() {
        assert_eq!(detect_version(&json!({"version": 3})), 3);
    }

    #[test]
    fn migrate_at_current_version_is_noop() {
        let input = json!({"version": CURRENT_VERSION, "lines": []});
        let (out, warnings) = migrate_value(input.clone(), CURRENT_VERSION).unwrap();
        assert_eq!(out, input);
        assert!(warnings.is_empty());
    }

    #[test]
    fn migrate_beyond_current_errors() {
        let input = json!({"version": 42});
        let err = migrate_value(input, 42).unwrap_err();
        assert!(matches!(err, MigrationError::UnknownVersion { .. }));
    }

    #[test]
    fn migrate_walks_registered_steps() {
        // Empty MIGRATIONS + source < CURRENT_VERSION: the forgiving path
        // bumps the version field in-place and carries on. Real migrations
        // insert themselves into the table and take over when they land.
        if MIGRATIONS.is_empty() && CURRENT_VERSION > 1 {
            let (out, warnings) = migrate_value(json!({"lines": []}), 1).unwrap();
            assert_eq!(
                out.get("version").and_then(Value::as_u64),
                Some(u64::from(CURRENT_VERSION))
            );
            assert!(warnings.is_empty());
        }
    }

    #[test]
    fn migrate_carries_version_forward_in_object() {
        let (out, _) = migrate_value(json!({"lines": [[]]}), 1).unwrap();
        // Version field should be bumped to CURRENT_VERSION on the way out.
        assert_eq!(
            out.get("version").and_then(Value::as_u64),
            Some(u64::from(CURRENT_VERSION))
        );
        // Other fields must be preserved.
        assert!(out.get("lines").is_some());
    }

    #[test]
    fn migrate_returns_empty_warnings_on_clean_input() {
        // No registered migrations means no warnings can be emitted yet, but
        // the shape must always be `(Value, Vec<_>)` — this test locks that in
        // so a future migration step that forgets to init the vec fails here.
        let (_, warnings) = migrate_value(json!({"lines": [[]]}), 1).unwrap();
        assert_eq!(warnings, Vec::new());
    }
}
