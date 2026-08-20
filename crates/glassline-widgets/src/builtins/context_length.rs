//! `context-length` — current context-window occupancy. Prefers
//! `StatusJson.context_window` metrics (authoritative live status); falls
//! back to the transcript-scan-derived `TokenMetrics.context_length`.
//! Port of TS `ContextLength.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{
    context_window_metrics, context_window_percent, format_tokens, is_raw, percent_hint_span,
    styled,
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ContextLength)
}

pub struct ContextLength;

impl Widget for ContextLength {
    fn id(&self) -> &'static str {
        "context-length"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let ctx_length = context_window_metrics(ctx.data.as_ref())
            .context_length_tokens
            .or_else(|| ctx.token_metrics.as_ref().map(|m| m.context_length));
        let Some(tokens) = ctx_length else {
            return Vec::new();
        };
        let formatted = format_tokens(tokens, 1);
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("Ctx: {formatted}")
        };
        let mut spans = styled(spec, text);
        // Widget's visible text has no `%` — attach the hint so
        // animate.rs `thresholds` / `pulseAbove` can fire on high context.
        if !spans.is_empty()
            && let Some(pct) = context_window_percent(ctx)
        {
            spans.push(percent_hint_span(pct));
        }
        spans
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{
        render_context::TokenMetrics,
        status_json::{ContextWindow, CurrentUsage, StatusJson},
    };

    #[test]
    fn uses_context_window_current_usage() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    context_window_size: Some(200_000.0),
                    current_usage: Some(CurrentUsage::Breakdown {
                        input_tokens: Some(30_000.0),
                        output_tokens: Some(5_000.0),
                        cache_creation_input_tokens: Some(10_000.0),
                        cache_read_input_tokens: Some(50_000.0),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextLength.render(&WidgetSpec::new("1", "context-length"), &ctx);
        // input + creation + read = 30k + 10k + 50k = 90k
        assert_eq!(spans[0].text, "Ctx: 90.0k");
    }

    #[test]
    fn falls_back_to_transcript_metrics() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 42_500,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextLength.render(&WidgetSpec::new("1", "context-length"), &ctx);
        assert_eq!(spans[0].text, "Ctx: 42.5k");
    }

    #[test]
    fn empty_when_nothing_available() {
        let spans = ContextLength.render(
            &WidgetSpec::new("1", "context-length"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn raw_value_drops_label() {
        let mut spec = WidgetSpec::new("1", "context-length");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 18_600,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextLength.render(&spec, &ctx);
        assert_eq!(spans[0].text, "18.6k");
    }
}
