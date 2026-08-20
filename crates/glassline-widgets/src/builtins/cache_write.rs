//! `cache-write` — cumulative cache-creation tokens (bytes written into
//! the prompt cache). Port of upstream `CacheWrite.ts`.

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
    Box::new(CacheWrite)
}

pub struct CacheWrite;

impl Widget for CacheWrite {
    fn id(&self) -> &'static str {
        "cache-write"
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
            labeled_or_raw(spec, "Cache write: ", &format_tokens(m.cache_creation, 1)),
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
    fn labels_cache_creation() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_creation: 8_400,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = CacheWrite.render(&WidgetSpec::new("1", "cache-write"), &ctx);
        assert_eq!(spans[0].text, "Cache write: 8.4k");
    }

    #[test]
    fn empty_without_metrics() {
        let spans = CacheWrite.render(
            &WidgetSpec::new("1", "cache-write"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
