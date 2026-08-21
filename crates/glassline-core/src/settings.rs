//! `settings.json` shape + WidgetSpec + PowerlineConfig + version field.
//!
//! Ports [`src/types/Settings.ts`](https://github.com/sirmalloc/ccstatusline) and
//! [`src/types/Widget.ts`](https://github.com/sirmalloc/ccstatusline).
//!
//! **Migration policy** (design §4.12): a settings file without a `version`
//! field is treated as `v1` (matches TS `SettingsSchema_v1`). Loaders should
//! walk the [`migration::MIGRATIONS`](crate::migration::MIGRATIONS) table until
//! they reach [`CURRENT_VERSION`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::color::ColorLevel;

/// Current on-disk schema version. Bump on every additive migration.
pub const CURRENT_VERSION: u32 = 3;

/// Top-level `settings.json` shape. `#[serde(default)]` on every field so a
/// half-populated file (or a legacy v1 file) still deserializes. Every
/// Option carries `skip_serializing_if = "Option::is_none"` so the
/// editor's atomic save writes only fields the user actually set —
/// otherwise every widget balloons to ~24 rows of `"foo": null`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Schema version. Legacy files without this field deserialize to `1`.
    pub version: u32,
    pub lines: Vec<Vec<WidgetSpec>>,
    pub flex_mode: FlexMode,
    pub compact_threshold: u32,
    pub color_level: ColorLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_separator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_padding: Option<String>,
    pub default_padding_side: DefaultPaddingSide,
    pub inherit_separator_colors: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_foreground_color: Option<String>,
    pub global_bold: bool,
    pub git_cache_ttl_seconds: u32,
    pub minimalist_mode: bool,
    pub powerline: PowerlineConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_message: Option<UpdateMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installation: Option<InstallationMetadata>,
    /// User-facing update-checker toggle (design §4.18). Default false.
    pub update_checker: UpdateCheckerSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            lines: default_lines(),
            flex_mode: FlexMode::FullMinus40,
            compact_threshold: 60,
            color_level: ColorLevel::Ansi256,
            default_separator: None,
            default_padding: None,
            default_padding_side: DefaultPaddingSide::Both,
            inherit_separator_colors: false,
            override_background_color: None,
            override_foreground_color: None,
            global_bold: false,
            git_cache_ttl_seconds: 5,
            minimalist_mode: false,
            powerline: PowerlineConfig::default(),
            update_message: None,
            installation: None,
            update_checker: UpdateCheckerSettings::default(),
        }
    }
}

impl Settings {
    /// The in-memory defaults chalk-parity users see on first run (mirror of
    /// TS `inMemoryDefaults()`).
    #[must_use]
    pub fn in_memory_defaults() -> Self {
        Self {
            version: CURRENT_VERSION,
            ..Self::default()
        }
    }
}

/// Baked-in fallback layout when no `settings.json` exists yet.
///
/// One populated line — model, context, branch, changes. Empty trailing
/// lines are omitted: the render pipeline emits one row per line
/// regardless of population, so ghost lines would print as blank rows in
/// the terminal. Users add lines via the editor; the empty vector here
/// keeps the default footprint honest.
fn default_lines() -> Vec<Vec<WidgetSpec>> {
    vec![vec![
        WidgetSpec::new("1", "model").with_color("cyan"),
        WidgetSpec::new("2", "separator"),
        WidgetSpec::new("3", "context-percentage").with_color("yellow"),
        WidgetSpec::new("4", "separator"),
        WidgetSpec::new("5", "git-branch").with_color("magenta"),
        WidgetSpec::new("6", "separator"),
        WidgetSpec::new("7", "git-changes").with_color("brightGreen"),
    ]]
}

/// A single widget entry in a line.
///
/// The TS schema accepts any `type` string for forward compatibility; we
/// preserve that so users can write `ext:*` types without recompilation. The
/// registry decides at load time whether the ID resolves to a built-in.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct WidgetSpec {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dim: Option<DimSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_value: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_colors: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge: Option<MergeSetting>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hide: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_from_auto_align: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<BTreeMap<String, String>>,

    // External-widget config (design §4.11).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_ttl_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_plaintext: Option<bool>,
}

impl WidgetSpec {
    #[must_use]
    pub fn new(id: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind: kind.into(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// `true` when this widget entry is an external-bin widget (per design
    /// §4.13 — `ext:` prefix is the only legal external namespace).
    #[must_use]
    pub fn is_external(&self) -> bool {
        self.kind.starts_with("ext:")
    }
}

/// `dim` accepts a bool or the string `"parens"`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum DimSetting {
    Bool(bool),
    Parens(ParensLiteral),
}

/// Marker for the literal `"parens"`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ParensLiteral {
    Parens,
}

/// `merge` accepts a bool or the string `"no-padding"`.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum MergeSetting {
    Bool(bool),
    NoPadding(NoPaddingLiteral),
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NoPaddingLiteral {
    NoPadding,
}

/// Which side(s) of a widget get default padding.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DefaultPaddingSide {
    #[default]
    Both,
    Left,
    Right,
}

/// Terminal-width flex handling mode.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum FlexMode {
    Full,
    #[default]
    #[serde(rename = "full-minus-40")]
    FullMinus40,
    #[serde(rename = "full-until-compact")]
    FullUntilCompact,
}

/// Powerline configuration.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct PowerlineConfig {
    pub enabled: bool,
    pub separators: Vec<String>,
    pub separator_invert_background: Vec<bool>,
    pub start_caps: Vec<String>,
    pub end_caps: Vec<String>,
    pub theme: Option<String>,
    pub auto_align: bool,
    pub continue_theme_across_lines: bool,
}

impl Default for PowerlineConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            separators: vec!["\u{E0B0}".to_string()],
            separator_invert_background: vec![false],
            start_caps: Vec::new(),
            end_caps: Vec::new(),
            theme: None,
            auto_align: false,
            continue_theme_across_lines: false,
        }
    }
}

/// Optional one-time upgrade message shown to the user (mirror of TS).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
pub struct UpdateMessage {
    pub message: Option<String>,
    pub remaining: Option<i64>,
}

/// User-facing update-checker toggle + cadence. The actual periodic
/// check isn't wired into the render binary yet — these fields are the
/// schema/UI half of the feature so users can persist their preferred
/// cadence in `settings.json` ahead of the implementation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateCheckerSettings {
    pub enabled: bool,
    /// Check every N hours since `last_check_epoch`. `None` = don't
    /// use interval-based scheduling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_hours: Option<u32>,
    /// Also check once per day at this local-time hour (0-23). `None`
    /// = don't use time-of-day scheduling. If both `interval_hours`
    /// and `daily_at_hour` are set, whichever fires first triggers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub daily_at_hour: Option<u8>,
    /// Unix epoch seconds of the last successful check. `None` = the
    /// implementation hasn't checked yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_epoch: Option<u64>,
}

/// How the CLI got installed. `method` is the tag.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum InstallationMetadata {
    AutoUpdate {
        #[serde(rename = "packageManager")]
        package_manager: KnownPackageManager,
    },
    Pinned {
        #[serde(rename = "installedVersion", skip_serializing_if = "Option::is_none")]
        installed_version: Option<String>,
    },
    SelfManaged {
        #[serde(rename = "packageManager", default)]
        package_manager: PackageManager,
    },
    Unknown {
        #[serde(rename = "packageManager", default)]
        package_manager: PackageManager,
    },
}

/// Package manager known to the auto-updater path.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum KnownPackageManager {
    Npm,
    Bun,
}

/// Package manager known to the self-managed/unknown path (adds a third arm
/// for unresolved detections).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Bun,
    #[default]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_gives_defaults() {
        let parsed: Settings = serde_json::from_str("{}").unwrap();
        // No `version` field ⇒ legacy v1 file.
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.color_level, ColorLevel::Ansi256);
        assert_eq!(parsed.flex_mode, FlexMode::FullMinus40);
    }

    #[test]
    fn explicit_version_preserved() {
        let parsed: Settings = serde_json::from_str(r#"{"version":3}"#).unwrap();
        assert_eq!(parsed.version, 3);
    }

    #[test]
    fn in_memory_defaults_are_current_version() {
        let s = Settings::in_memory_defaults();
        assert_eq!(s.version, CURRENT_VERSION);
    }

    #[test]
    fn widget_spec_ext_prefix_detected() {
        let ext = WidgetSpec::new("id1", "ext:my-widget");
        assert!(ext.is_external());
        let builtin = WidgetSpec::new("id2", "git-branch");
        assert!(!builtin.is_external());
    }

    #[test]
    fn flex_mode_kebab_case() {
        assert_eq!(
            serde_json::to_string(&FlexMode::FullMinus40).unwrap(),
            "\"full-minus-40\""
        );
        let parsed: FlexMode = serde_json::from_str("\"full-until-compact\"").unwrap();
        assert_eq!(parsed, FlexMode::FullUntilCompact);
    }

    #[test]
    fn installation_metadata_tag_dispatch() {
        let auto: InstallationMetadata =
            serde_json::from_str(r#"{"method":"auto-update","packageManager":"npm"}"#).unwrap();
        assert!(matches!(
            auto,
            InstallationMetadata::AutoUpdate {
                package_manager: KnownPackageManager::Npm
            }
        ));
    }

    #[test]
    fn dim_accepts_bool_and_literal() {
        let bool_dim: WidgetSpec =
            serde_json::from_str(r#"{"id":"a","type":"custom-text","dim":true}"#).unwrap();
        assert!(matches!(bool_dim.dim, Some(DimSetting::Bool(true))));
        let parens_dim: WidgetSpec =
            serde_json::from_str(r#"{"id":"a","type":"custom-text","dim":"parens"}"#).unwrap();
        assert!(matches!(
            parens_dim.dim,
            Some(DimSetting::Parens(ParensLiteral::Parens))
        ));
    }

    #[test]
    fn merge_accepts_bool_and_literal() {
        let no_pad: WidgetSpec =
            serde_json::from_str(r#"{"id":"a","type":"custom-text","merge":"no-padding"}"#)
                .unwrap();
        assert!(matches!(
            no_pad.merge,
            Some(MergeSetting::NoPadding(NoPaddingLiteral::NoPadding))
        ));
    }

    #[test]
    fn round_trip_default() {
        let s = Settings::in_memory_defaults();
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
