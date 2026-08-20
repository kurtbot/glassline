//! `tokens-cached` — `cache_read + cache_creation` from transcript
//! scanner. Port of upstream `TokensCached.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{format_tokens, labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(TokensCached)
}

pub struct TokensCached;

impl Widget for TokensCached {
    fn id(&self) -> &'static str {
        "tokens-cached"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(m) = ctx.token_metrics.as_ref() else {
            return Vec::new();
        };
        styled(
            spec,
            labeled_or_raw(spec, "Cached: ", &format_tokens(m.cached(), 1)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::TokenMetrics;

    #[test]
    fn sums_read_and_creation() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_read: 40_000,
                cache_creation: 10_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensCached.render(&WidgetSpec::new("1", "tokens-cached"), &ctx);
        assert_eq!(spans[0].text, "Cached: 50.0k");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "tokens-cached");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_read: 2_500,
                cache_creation: 500,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = TokensCached.render(&spec, &ctx);
        assert_eq!(spans[0].text, "3.0k");
    }

    #[test]
    fn empty_without_metrics() {
        let spans = TokensCached.render(
            &WidgetSpec::new("1", "tokens-cached"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
