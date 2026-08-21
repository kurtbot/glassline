//! `tokens-total` — `input + output + cache_read + cache_creation`.
//! Port of upstream `TokensTotal.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{
    context_window_percent, format_tokens, labeled_or_raw, percent_hint_span, styled,
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(TokensTotal)
}

pub struct TokensTotal;

impl Widget for TokensTotal {
    fn id(&self) -> &'static str {
        "tokens-total"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(m) = ctx.token_metrics.as_ref() else {
            return Vec::new();
        };
        let mut spans = styled(
            spec,
            labeled_or_raw(spec, "Total: ", &format_tokens(m.total(), 1)),
        );
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
    use glassline_core::render_context::TokenMetrics;

    #[test]
    fn sums_all_four_buckets() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                input: 100_000,
                output: 25_000,
                cache_read: 40_000,
                cache_creation: 10_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensTotal.render(&WidgetSpec::new("1", "tokens-total"), &ctx);
        assert_eq!(spans[0].text, "Total: 175.0k");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "tokens-total");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                input: 1_000,
                output: 500,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensTotal.render(&spec, &ctx);
        assert_eq!(spans[0].text, "1.5k");
    }

    #[test]
    fn empty_without_metrics() {
        let spans = TokensTotal.render(
            &WidgetSpec::new("1", "tokens-total"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
