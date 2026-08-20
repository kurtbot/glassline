//! `session-name` — Claude Code's human-friendly session label.
//! Port of upstream `SessionName.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(SessionName)
}

pub struct SessionName;

impl Widget for SessionName {
    fn id(&self) -> &'static str {
        "session-name"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(name) = ctx
            .data
            .as_ref()
            .and_then(|d| d.session_name.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        styled(spec, name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::StatusJson;

    fn ctx(name: Option<&str>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                session_name: name.map(String::from),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn renders_session_name() {
        let spans = SessionName.render(
            &WidgetSpec::new("1", "session-name"),
            &ctx(Some("refactor-usage")),
        );
        assert_eq!(spans[0].text, "refactor-usage");
    }

    #[test]
    fn empty_when_absent() {
        let spans = SessionName.render(&WidgetSpec::new("1", "session-name"), &ctx(None));
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_empty_string() {
        let spans = SessionName.render(&WidgetSpec::new("1", "session-name"), &ctx(Some("")));
        assert!(spans.is_empty());
    }
}
