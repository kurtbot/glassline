//! `cache-read` — cumulative cache-read tokens. Port of upstream `CacheRead.ts`.

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
    Box::new(CacheRead)
}

pub struct CacheRead;

impl Widget for CacheRead {
    fn id(&self) -> &'static str {
        "cache-read"
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
        let mut spans = styled(
            spec,
            labeled_or_raw(spec, "Cache read: ", &format_tokens(m.cache_read, 1)),
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
    fn labels_cache_read() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_read: 42_500,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = CacheRead.render(&WidgetSpec::new("1", "cache-read"), &ctx);
        assert_eq!(spans[0].text, "Cache read: 42.5k");
    }

    #[test]
    fn empty_without_metrics() {
        let spans = CacheRead.render(
            &WidgetSpec::new("1", "cache-read"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
