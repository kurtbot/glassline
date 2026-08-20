//! `cache-hit-rate` — percentage of input-tokens served from the prompt
//! cache: `cache_read / (cache_read + input)`. Port of upstream
//! `CacheHitRate.ts`.
//!
//! The denominator uses `cache_read + input` because those are the two
//! ways input tokens enter a turn (cache hit vs full send). Cache writes
//! (`cache_creation`) are one-time costs for populating the cache — they
//! don't belong in the hit-rate denominator.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(CacheHitRate)
}

pub struct CacheHitRate;

impl Widget for CacheHitRate {
    fn id(&self) -> &'static str {
        "cache-hit-rate"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(m) = ctx.token_metrics.as_ref() else {
            return Vec::new();
        };
        let denom = m.cache_read + m.input;
        if denom == 0 {
            return Vec::new();
        }
        let pct = (m.cache_read as f64 / denom as f64) * 100.0;
        let formatted = format!("{pct:.1}%");
        styled(spec, labeled_or_raw(spec, "Cache hit: ", &formatted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::TokenMetrics;

    #[test]
    fn computes_hit_rate_from_totals() {
        // 90k cache reads, 10k fresh input → 90 / 100 = 90.0%.
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_read: 90_000,
                input: 10_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = CacheHitRate.render(&WidgetSpec::new("1", "cache-hit-rate"), &ctx);
        assert_eq!(spans[0].text, "Cache hit: 90.0%");
    }

    #[test]
    fn zero_percent_when_no_cache_reads() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_read: 0,
                input: 10_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = CacheHitRate.render(&WidgetSpec::new("1", "cache-hit-rate"), &ctx);
        assert_eq!(spans[0].text, "Cache hit: 0.0%");
    }

    #[test]
    fn empty_when_denominator_zero() {
        // No usage at all → suppress the widget rather than emit "NaN%".
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics::default()),
            ..Default::default()
        };
        let spans = CacheHitRate.render(&WidgetSpec::new("1", "cache-hit-rate"), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_without_metrics() {
        let spans = CacheHitRate.render(
            &WidgetSpec::new("1", "cache-hit-rate"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn cache_creation_does_not_affect_denominator() {
        // Same reads + input, but with cache_creation set — hit rate is
        // unchanged. (`cache_creation` is one-time write cost, not a
        // denominator entry.)
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                cache_read: 40_000,
                input: 10_000,
                cache_creation: 50_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = CacheHitRate.render(&WidgetSpec::new("1", "cache-hit-rate"), &ctx);
        assert_eq!(spans[0].text, "Cache hit: 80.0%");
    }
}
