//! `tokens-output` — cumulative output token count.
//! Prefers `TokenMetrics.output`; falls back to
//! `context_window.total_output_tokens`. Port of TS `TokensOutput.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{context_window_metrics, format_tokens, labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(TokensOutput)
}

pub struct TokensOutput;

impl Widget for TokensOutput {
    fn id(&self) -> &'static str {
        "tokens-output"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("white")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let tokens = ctx
            .token_metrics
            .as_ref()
            .map(|m| m.output)
            .or_else(|| context_window_metrics(ctx.data.as_ref()).total_output_tokens);
        let Some(t) = tokens else {
            return Vec::new();
        };
        styled(spec, labeled_or_raw(spec, "Out: ", &format_tokens(t, 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::TokenMetrics;

    #[test]
    fn labels_transcript_derived_output() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                output: 3_400,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensOutput.render(&WidgetSpec::new("1", "tokens-output"), &ctx);
        assert_eq!(spans[0].text, "Out: 3.4k");
    }

    #[test]
    fn raw_drops_prefix() {
        let mut spec = WidgetSpec::new("1", "tokens-output");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                output: 8_100,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensOutput.render(&spec, &ctx);
        assert_eq!(spans[0].text, "8.1k");
    }

    #[test]
    fn empty_when_nothing_available() {
        let spans = TokensOutput.render(
            &WidgetSpec::new("1", "tokens-output"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
