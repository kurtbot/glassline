//! `context-window` — labeled `used / total` tokens. Port of upstream
//! `ContextWindow.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{
    context_window_metrics, default_context_window_size, format_tokens, is_raw, styled,
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ContextWindowWidget)
}

pub struct ContextWindowWidget;

impl Widget for ContextWindowWidget {
    fn id(&self) -> &'static str {
        "context-window"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let metrics = context_window_metrics(ctx.data.as_ref());
        let total = metrics
            .window_size
            .unwrap_or_else(default_context_window_size);
        let used = metrics
            .context_length_tokens
            .or_else(|| ctx.token_metrics.as_ref().map(|m| m.context_length));
        let Some(used) = used else {
            return Vec::new();
        };
        let value = format!("{}/{}", format_tokens(used, 0), format_tokens(total, 0));
        let text = if is_raw(spec) {
            value
        } else {
            format!("Ctx: {value}")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{
        render_context::TokenMetrics,
        status_json::{ContextWindow, StatusJson},
    };

    #[test]
    fn labels_used_and_total() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    context_window_size: Some(200_000.0),
                    used_percentage: Some(50.0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextWindowWidget.render(&WidgetSpec::new("1", "context-window"), &ctx);
        assert_eq!(spans[0].text, "Ctx: 100k/200k");
    }

    #[test]
    fn falls_back_to_transcript() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 80_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextWindowWidget.render(&WidgetSpec::new("1", "context-window"), &ctx);
        assert_eq!(spans[0].text, "Ctx: 80k/200k");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "context-window");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 100_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextWindowWidget.render(&spec, &ctx);
        assert_eq!(spans[0].text, "100k/200k");
    }

    #[test]
    fn empty_when_no_usage_available() {
        let spans = ContextWindowWidget.render(
            &WidgetSpec::new("1", "context-window"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
