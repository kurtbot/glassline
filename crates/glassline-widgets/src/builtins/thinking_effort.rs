//! `thinking-effort` — renders the current effort level (low/medium/high/
//! xhigh/max) or `default` when unknown. Port of TS `ThinkingEffort.ts`.
//!
//! **MVP scope:** honours `StatusJson.effort.level` only. TS's fallback
//! chain — transcript last-line `<local-command-stdout>Set effort level
//! to X` scan and `~/.claude/settings.json`'s `effortLevel` field — is
//! deferred to T-1.7d.

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
            .and_then(|e| e.level.clone());
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
        let spans = ThinkingEffort.render(&WidgetSpec::new("1", "thinking-effort"), &ctx(None));
        assert_eq!(spans[0].text, "Thinking: default");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "thinking-effort");
        spec.raw_value = Some(true);
        let spans = ThinkingEffort.render(&spec, &ctx(Some("medium")));
        assert_eq!(spans[0].text, "medium");
    }

    #[test]
    fn junk_input_renders_default() {
        let spans =
            ThinkingEffort.render(&WidgetSpec::new("1", "thinking-effort"), &ctx(Some("!!!")));
        assert_eq!(spans[0].text, "Thinking: default");
    }
}
