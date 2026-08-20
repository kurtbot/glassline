//! `context-percentage-usable` — percentage of the context window used
//! against the "usable" ceiling (context_window_size minus the model's
//! reserved max-output allocation). Port of upstream
//! `ContextPercentageUsable.ts`.
//!
//! Reads `StatusJson.context_window.usable_percentage` directly. Claude
//! Code populates this only when it knows the model's output cap; when
//! absent, the widget renders nothing rather than guessing.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ContextPercentageUsable)
}

pub struct ContextPercentageUsable;

impl Widget for ContextPercentageUsable {
    fn id(&self) -> &'static str {
        "context-percentage-usable"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(pct) = ctx
            .data
            .as_ref()
            .and_then(|d| d.context_window.as_ref())
            .and_then(|cw| cw.usable_percentage)
        else {
            return Vec::new();
        };
        let clamped = pct.clamp(0.0, 100.0);
        styled(
            spec,
            labeled_or_raw(spec, "Ctx Usable: ", &format!("{clamped:.1}%")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{ContextWindow, StatusJson};

    fn ctx(pct: Option<f64>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    usable_percentage: pct,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labels_percent() {
        let spans = ContextPercentageUsable.render(
            &WidgetSpec::new("1", "context-percentage-usable"),
            &ctx(Some(9.3)),
        );
        assert_eq!(spans[0].text, "Ctx Usable: 9.3%");
    }

    #[test]
    fn clamps_over_100() {
        let spans = ContextPercentageUsable.render(
            &WidgetSpec::new("1", "context-percentage-usable"),
            &ctx(Some(150.0)),
        );
        assert_eq!(spans[0].text, "Ctx Usable: 100.0%");
    }

    #[test]
    fn empty_when_absent() {
        let spans = ContextPercentageUsable.render(
            &WidgetSpec::new("1", "context-percentage-usable"),
            &ctx(None),
        );
        assert!(spans.is_empty());
    }
}
