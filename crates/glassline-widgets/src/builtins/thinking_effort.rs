//! `thinking-effort` — renders the current effort level (low/medium/high/
//! xhigh/max) or `default` when unknown. Port of TS `ThinkingEffort.ts`.
//!
//! # Fallback chain
//!
//! 1. `StatusJson.effort.level` (Claude Code's authoritative value).
//! 2. `ctx.last_effort_level` — populated by the transcript scanner from
//!    a `<local-command-stdout>Set effort level to X` marker. `None`
//!    until the scanner extension lands.
//! 3. `~/.claude/settings.json` `effortLevel` field — read on demand.
//! 4. Literal `default`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ThinkingEffort)
}

pub struct ThinkingEffort;

const KNOWN: &[&str] = &["low", "medium", "high", "xhigh", "max"];

impl Widget for ThinkingEffort {
    fn id(&self) -> &'static str {
        "thinking-effort"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let raw_level = ctx
            .data
            .as_ref()
            .and_then(|d| d.effort.as_ref())
            .and_then(|e| e.level.clone())
            .or_else(|| ctx.last_effort_level.clone())
            .or_else(read_effort_from_settings_json);
        let effort = match raw_level.map(|s| s.to_lowercase()) {
            None => "default".to_string(),
            Some(level) if KNOWN.contains(&level.as_str()) => level,
            Some(level) if is_unknown_effort_shape(&level) => format!("{level}?"),
            Some(_) => "default".to_string(),
        };
        let text = if is_raw(spec) {
            effort
        } else {
            format!("Thinking: {effort}")
        };
        styled(spec, text)
    }
}

/// Read the `effortLevel` field from `~/.claude/settings.json`. `None`
/// when the file doesn't exist, is malformed, or lacks the field.
fn read_effort_from_settings_json() -> Option<String> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    let path = std::path::PathBuf::from(home)
        .join(".claude")
        .join("settings.json");
    let raw = std::fs::read_to_string(&path).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&raw).ok()?;
    parsed
        .get("effortLevel")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Mirror of TS `UNKNOWN_EFFORT_PATTERN = /^(?=.*[a-z0-9])[a-z0-9-]{2,20}$/`.
/// Any 2-20 char string with at least one alphanumeric passes.
fn is_unknown_effort_shape(s: &str) -> bool {
    if !(2..=20).contains(&s.len()) {
        return false;
    }
    let mut has_alnum = false;
    for c in s.chars() {
        if !(c.is_ascii_alphanumeric() || c == '-') {
            return false;
        }
        if c.is_ascii_alphanumeric() {
            has_alnum = true;
        }
    }
    has_alnum
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{Effort, StatusJson};

    fn ctx(level: Option<&str>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                effort: Some(Effort {
                    level: level.map(String::from),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn known_levels_render_as_labeled() {
        for level in ["low", "medium", "high", "xhigh", "max"] {
            let spans =
                ThinkingEffort.render(&WidgetSpec::new("1", "thinking-effort"), &ctx(Some(level)));
            assert_eq!(spans[0].text, format!("Thinking: {level}"));
        }
    }

    #[test]
    fn unknown_levels_get_question_suffix() {
        let spans = ThinkingEffort.render(
            &WidgetSpec::new("1", "thinking-effort"),
            &ctx(Some("super-max")),
        );
        assert_eq!(spans[0].text, "Thinking: super-max?");
    }

    #[test]
    fn absent_level_renders_default() {
        // Force HOME/USERPROFILE to a nonexistent dir so the new
        // settings.json fallback (~/.claude/settings.json) misses and
        // the widget falls through to the literal "default".
        let _guard = crate::common::TEST_ENV_LOCK.lock().unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_up = std::env::var_os("USERPROFILE");
        unsafe {
            std::env::set_var("HOME", "/no/such/dir/glassline-test");
            std::env::set_var("USERPROFILE", "/no/such/dir/glassline-test");
        }
        let spans = ThinkingEffort.render(&WidgetSpec::new("1", "thinking-effort"), &ctx(None));
        unsafe {
            match saved_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match saved_up {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
        }
        assert_eq!(spans[0].text, "Thinking: default");
    }

    #[test]
    fn raw_drops_label() {
        let _guard = crate::common::TEST_ENV_LOCK.lock().unwrap();
        let mut spec = WidgetSpec::new("1", "thinking-effort");
        spec.raw_value = Some(true);
        let spans = ThinkingEffort.render(&spec, &ctx(Some("medium")));
        assert_eq!(spans[0].text, "medium");
    }

    #[test]
    fn junk_input_renders_default() {
        // Same env-var guard as absent_level_renders_default — junk
        // level → settings.json fallback runs → must miss.
        let _guard = crate::common::TEST_ENV_LOCK.lock().unwrap();
        let saved_home = std::env::var_os("HOME");
        let saved_up = std::env::var_os("USERPROFILE");
        unsafe {
            std::env::set_var("HOME", "/no/such/dir/glassline-test");
            std::env::set_var("USERPROFILE", "/no/such/dir/glassline-test");
        }
        let spans =
            ThinkingEffort.render(&WidgetSpec::new("1", "thinking-effort"), &ctx(Some("!!!")));
        unsafe {
            match saved_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match saved_up {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
        }
        assert_eq!(spans[0].text, "Thinking: default");
    }
}
