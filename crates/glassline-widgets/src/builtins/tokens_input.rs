//! `tokens-input` — cumulative input token count.
//! Prefers `TokenMetrics.input` (transcript-derived); falls back to
//! `context_window.total_input_tokens`. Port of TS `TokensInput.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{context_window_metrics, format_tokens, labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(TokensInput)
}

pub struct TokensInput;

impl Widget for TokensInput {
    fn id(&self) -> &'static str {
        "tokens-input"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let tokens = ctx
            .token_metrics
            .as_ref()
            .map(|m| m.input)
            .or_else(|| context_window_metrics(ctx.data.as_ref()).total_input_tokens);
        let Some(t) = tokens else {
            return Vec::new();
        };
        styled(spec, labeled_or_raw(spec, "In: ", &format_tokens(t, 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::TokenMetrics;

    #[test]
    fn labels_transcript_derived_input() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                input: 15_200,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensInput.render(&WidgetSpec::new("1", "tokens-input"), &ctx);
        assert_eq!(spans[0].text, "In: 15.2k");
    }

    #[test]
    fn empty_when_nothing_available() {
        let spans = TokensInput.render(
            &WidgetSpec::new("1", "tokens-input"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn raw_drops_prefix() {
        let mut spec = WidgetSpec::new("1", "tokens-input");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                input: 42_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensInput.render(&spec, &ctx);
        assert_eq!(spans[0].text, "42.0k");
    }
}
