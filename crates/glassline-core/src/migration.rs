//! Settings-schema migration table (design §4.12).
//!
//! Migrations run on load: each [`Migration`] rewrites a `Value` from its
//! `from` version to `from + 1`. The migration engine is deliberately generic
//! ([`migrate_value`]) — v2→v3 (custom-command → ext:) lands in P2 alongside
//! the [`glassline-ext`] crate.

use serde_json::Value;
use thiserror::Error;

use crate::settings::CURRENT_VERSION;

/// One version-step migration.
pub struct Migration {
    pub from: u32,
    pub to: u32,
    /// Rewrite raw `Value` in place. Called only when the source file is at
    /// [`Self::from`]; expected to produce a value that deserializes as
    /// [`Self::to`].
    pub apply: fn(Value) -> Result<Value, MigrationError>,
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

/// Run every registered migration whose `from` is ≥ `source_version`.
///
/// Returns the JSON value at [`CURRENT_VERSION`]. The caller is responsible
/// for reading the `version` field of the input and passing it here.
pub fn migrate_value(mut value: Value, source_version: u32) -> Result<Value, MigrationError> {
    if source_version > CURRENT_VERSION {
        return Err(MigrationError::UnknownVersion {
            version: source_version,
            max: CURRENT_VERSION,
        });
    }
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
        value = (migration.apply)(value)?;
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
    Ok(value)
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
        let out = migrate_value(input.clone(), CURRENT_VERSION).unwrap();
        assert_eq!(out, input);
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
            let out = migrate_value(json!({"lines": []}), 1).unwrap();
            assert_eq!(
                out.get("version").and_then(Value::as_u64),
                Some(u64::from(CURRENT_VERSION))
            );
        }
    }

    #[test]
    fn migrate_carries_version_forward_in_object() {
        let out = migrate_value(json!({"lines": [[]]}), 1).unwrap();
        // Version field should be bumped to CURRENT_VERSION on the way out.
        assert_eq!(
            out.get("version").and_then(Value::as_u64),
            Some(u64::from(CURRENT_VERSION))
        );
        // Other fields must be preserved.
        assert!(out.get("lines").is_some());
    }
}
