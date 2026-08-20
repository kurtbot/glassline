//! Serde mirror of Claude Code's `StatusJSON` payload.
//!
//! Ports [`src/types/StatusJSON.ts`](https://github.com/sirmalloc/ccstatusline) —
//! a `z.looseObject`. Every field is optional; unknown fields land in
//! [`StatusJson::extras`] via `#[serde(flatten)]`.
//!
//! Numeric fields go through [`coerced_number`] to accept both `42` and `"42"`
//! (TS zod uses `CoercedNumberSchema`). Coercion failures return `None` — the
//! widget layer then treats missing values as "no data" rather than crashing.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// The top-level payload Claude Code writes to `glassline` on stdin.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct StatusJson {
    pub hook_event_name: Option<String>,
    pub session_id: Option<String>,
    /// Human-friendly session label (Claude Code sets this when the user
    /// or the tool has named the session). Optional.
    pub session_name: Option<String>,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<ModelInfo>,
    pub workspace: Option<Workspace>,
    pub version: Option<String>,
    pub output_style: Option<OutputStyle>,
    pub effort: Option<Effort>,
    pub cost: Option<Cost>,
    pub context_window: Option<ContextWindow>,
    pub vim: Option<Vim>,
    pub worktree: Option<Worktree>,
    pub rate_limits: Option<RateLimits>,

    /// Unknown fields — Claude Code may add new keys at any time. We keep them
    /// as opaque `Value`s so a widget written against a future protocol can
    /// read them via [`StatusJson::extras`] without breaking older widgets.
    #[serde(flatten)]
    pub extras: BTreeMap<String, Value>,
}

/// `model` in `StatusJSON` is either a raw string or an object.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum ModelInfo {
    Name(String),
    Full {
        id: Option<String>,
        display_name: Option<String>,
    },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Workspace {
    pub current_dir: Option<String>,
    pub project_dir: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OutputStyle {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Effort {
    pub level: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Cost {
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_cost_usd: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_duration_ms: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_api_duration_ms: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_lines_added: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_lines_removed: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct ContextWindow {
    #[serde(deserialize_with = "coerced_number", default)]
    pub context_window_size: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_input_tokens: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub total_output_tokens: Option<f64>,
    pub current_usage: Option<CurrentUsage>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub used_percentage: Option<f64>,
    #[serde(deserialize_with = "coerced_number", default)]
    pub remaining_percentage: Option<f64>,
    /// Percentage of the context window that's still usable after
    /// reserving the model's max-output allocation. Claude Code populates
    /// this when it knows the model's output cap; absent otherwise.
    #[serde(deserialize_with = "coerced_number", default)]
    pub usable_percentage: Option<f64>,
}

/// `current_usage` is either a bare number or a token breakdown object.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum CurrentUsage {
    Number(f64),
    Breakdown {
        #[serde(deserialize_with = "coerced_number", default)]
        input_tokens: Option<f64>,
        #[serde(deserialize_with = "coerced_number", default)]
        output_tokens: Option<f64>,
        #[serde(deserialize_with = "coerced_number", default)]
        cache_creation_input_tokens: Option<f64>,
        #[serde(deserialize_with = "coerced_number", default)]
        cache_read_input_tokens: Option<f64>,
    },
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Vim {
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Worktree {
    pub name: Option<String>,
    pub path: Option<String>,
    pub branch: Option<String>,
    pub original_cwd: Option<String>,
    pub original_branch: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct RateLimits {
    pub five_hour: Option<RateLimitPeriod>,
    pub seven_day: Option<RateLimitPeriod>,
    pub seven_day_sonnet: Option<RateLimitPeriod>,
    pub seven_day_opus: Option<RateLimitPeriod>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct RateLimitPeriod {
    #[serde(deserialize_with = "coerced_number", default)]
    pub used_percentage: Option<f64>,
    /// Unix epoch seconds.
    #[serde(deserialize_with = "coerced_number", default)]
    pub resets_at: Option<f64>,
}

/// Deserialize a `Value` as `f64`, coercing `"42"` → `42.0`.
///
/// Returns `None` when the value is `null`, an empty/whitespace-only string,
/// or a non-numeric string. Never errors — the TS layer treats every parse
/// failure as "field absent", and so do we.
fn coerced_number<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(coerce_value_to_f64(value))
}

fn coerce_value_to_f64(value: Option<Value>) -> Option<f64> {
    match value? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_parses_to_default() {
        let parsed: StatusJson = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, StatusJson::default());
    }

    #[test]
    fn model_accepts_string() {
        let parsed: StatusJson = serde_json::from_str(r#"{"model":"claude-opus-4-7"}"#).unwrap();
        assert!(matches!(parsed.model, Some(ModelInfo::Name(ref s)) if s == "claude-opus-4-7"));
    }

    #[test]
    fn model_accepts_object() {
        let parsed: StatusJson =
            serde_json::from_str(r#"{"model":{"id":"claude-opus-4-7","display_name":"Opus 4.7"}}"#)
                .unwrap();
        match parsed.model {
            Some(ModelInfo::Full { id, display_name }) => {
                assert_eq!(id.as_deref(), Some("claude-opus-4-7"));
                assert_eq!(display_name.as_deref(), Some("Opus 4.7"));
            }
            other => panic!("expected Full model, got {other:?}"),
        }
    }

    #[test]
    fn coerced_number_accepts_number() {
        let parsed: StatusJson =
            serde_json::from_str(r#"{"cost":{"total_cost_usd":1.5}}"#).unwrap();
        assert_eq!(parsed.cost.and_then(|c| c.total_cost_usd), Some(1.5));
    }

    #[test]
    fn coerced_number_accepts_string() {
        let parsed: StatusJson =
            serde_json::from_str(r#"{"cost":{"total_cost_usd":"1.5"}}"#).unwrap();
        assert_eq!(parsed.cost.and_then(|c| c.total_cost_usd), Some(1.5));
    }

    #[test]
    fn coerced_number_rejects_garbage_as_none() {
        let parsed: StatusJson =
            serde_json::from_str(r#"{"cost":{"total_cost_usd":"nope"}}"#).unwrap();
        assert_eq!(parsed.cost.and_then(|c| c.total_cost_usd), None);
    }

    #[test]
    fn current_usage_accepts_number() {
        let parsed: StatusJson =
            serde_json::from_str(r#"{"context_window":{"current_usage":42000}}"#).unwrap();
        assert!(matches!(
            parsed.context_window.and_then(|cw| cw.current_usage),
            Some(CurrentUsage::Number(n)) if (n - 42000.0).abs() < f64::EPSILON,
        ));
    }

    #[test]
    fn current_usage_accepts_breakdown() {
        let parsed: StatusJson = serde_json::from_str(
            r#"{"context_window":{"current_usage":{"input_tokens":100,"output_tokens":50}}}"#,
        )
        .unwrap();
        match parsed.context_window.and_then(|cw| cw.current_usage) {
            Some(CurrentUsage::Breakdown {
                input_tokens,
                output_tokens,
                ..
            }) => {
                assert_eq!(input_tokens, Some(100.0));
                assert_eq!(output_tokens, Some(50.0));
            }
            other => panic!("expected Breakdown, got {other:?}"),
        }
    }

    #[test]
    fn unknown_fields_land_in_extras() {
        let parsed: StatusJson =
            serde_json::from_str(r#"{"session_id":"abc","future_field":"hello"}"#).unwrap();
        assert_eq!(parsed.session_id.as_deref(), Some("abc"));
        assert_eq!(
            parsed.extras.get("future_field").and_then(Value::as_str),
            Some("hello")
        );
    }

    #[test]
    fn round_trip_preserves_shape() {
        let json = r#"{"session_id":"abc","cwd":"/tmp","model":"claude"}"#;
        let parsed: StatusJson = serde_json::from_str(json).unwrap();
        let reserialized = serde_json::to_value(&parsed).unwrap();
        assert_eq!(
            reserialized.get("session_id").and_then(Value::as_str),
            Some("abc")
        );
        assert_eq!(
            reserialized.get("cwd").and_then(Value::as_str),
            Some("/tmp")
        );
    }
}
