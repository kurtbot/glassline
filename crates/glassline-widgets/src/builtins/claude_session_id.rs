//! `claude-session-id` — Claude Code session identifier, truncated to
//! 8 chars by default (matches upstream `ClaudeSessionId.ts`).
//!
//! Configurable via `metadata.length` (u32) — accepts 0..=64. Longer
//! values than the actual ID clamp to the full ID length.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

const DEFAULT_LENGTH: usize = 8;

pub fn factory() -> Box<dyn Widget> {
    Box::new(ClaudeSessionId)
}

pub struct ClaudeSessionId;

impl Widget for ClaudeSessionId {
    fn id(&self) -> &'static str {
        "claude-session-id"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(id) = ctx.data.as_ref().and_then(|d| d.session_id.as_ref()) else {
            return Vec::new();
        };
        let n = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("length"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_LENGTH)
            .min(64);
        let short: String = id.chars().take(n).collect();
        styled(spec, labeled_or_raw(spec, "Session: ", &short))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::StatusJson;

    fn ctx(id: Option<&str>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                session_id: id.map(String::from),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn default_truncates_to_8() {
        let spans = ClaudeSessionId.render(
            &WidgetSpec::new("1", "claude-session-id"),
            &ctx(Some("abcdef0123456789")),
        );
        assert_eq!(spans[0].text, "Session: abcdef01");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "claude-session-id");
        spec.raw_value = Some(true);
        let spans = ClaudeSessionId.render(&spec, &ctx(Some("abcdef0123456789")));
        assert_eq!(spans[0].text, "abcdef01");
    }

    #[test]
    fn metadata_length_override() {
        let mut spec = WidgetSpec::new("1", "claude-session-id");
        spec.metadata = Some(
            [("length".to_string(), "4".to_string())]
                .into_iter()
                .collect(),
        );
        let spans = ClaudeSessionId.render(&spec, &ctx(Some("abcdef01")));
        assert_eq!(spans[0].text, "Session: abcd");
    }

    #[test]
    fn empty_when_no_session_id() {
        let spans = ClaudeSessionId.render(&WidgetSpec::new("1", "claude-session-id"), &ctx(None));
        assert!(spans.is_empty());
    }

    #[test]
    fn short_id_returns_whole_id() {
        // 4-char id + default length=8 → returns the whole 4-char id.
        let spans = ClaudeSessionId.render(
            &WidgetSpec::new("1", "claude-session-id"),
            &ctx(Some("abcd")),
        );
        assert_eq!(spans[0].text, "Session: abcd");
    }
}
