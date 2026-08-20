//! `block-reset-timer` — time until the current 5-hour billing block resets.
//! Port of upstream `BlockResetTimer.ts`.
//!
//! Reads `ctx.block_metrics.resets_at` (RFC3339). If absent or in the past,
//! renders `0m`. If unparseable, renders nothing.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{DurationFormat, duration_until_iso_ms, format_duration_ms, is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(BlockResetTimer)
}

pub struct BlockResetTimer;

impl Widget for BlockResetTimer {
    fn id(&self) -> &'static str {
        "block-reset-timer"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::BLOCK
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(resets_at) = ctx
            .block_metrics
            .as_ref()
            .and_then(|b| b.resets_at.as_deref())
        else {
            return Vec::new();
        };
        let Some(ms) = duration_until_iso_ms(resets_at) else {
            return Vec::new();
        };
        // Block windows never span days, but keep use_days: true to match
        // the shared usage-timer default in case a session survives across
        // an unusually long clock jump.
        let formatted = format_duration_ms(ms, DurationFormat::default());
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("Block reset: {formatted}")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::BlockMetrics;

    #[test]
    fn empty_when_block_metrics_absent() {
        let spans = BlockResetTimer.render(
            &WidgetSpec::new("1", "block-reset-timer"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_resets_at_unparseable() {
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics {
                resets_at: Some("not-a-date".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = BlockResetTimer.render(&WidgetSpec::new("1", "block-reset-timer"), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn zero_when_resets_in_past() {
        let ts = time::OffsetDateTime::now_utc() - time::Duration::hours(1);
        let iso = ts
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics {
                resets_at: Some(iso),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = BlockResetTimer.render(&WidgetSpec::new("1", "block-reset-timer"), &ctx);
        assert_eq!(spans[0].text, "Block reset: 0m");
    }

    #[test]
    fn labeled_when_resets_in_future() {
        let ts = time::OffsetDateTime::now_utc() + time::Duration::hours(2);
        let iso = ts
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics {
                resets_at: Some(iso),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = BlockResetTimer.render(&WidgetSpec::new("1", "block-reset-timer"), &ctx);
        assert!(spans[0].text.starts_with("Block reset: "));
        // Expected roughly "1hr 59m" or "2hr" depending on clock granularity.
        assert!(
            spans[0].text.contains("1hr") || spans[0].text.contains("2hr"),
            "got {}",
            spans[0].text
        );
    }

    #[test]
    fn raw_drops_label() {
        let ts = time::OffsetDateTime::now_utc() + time::Duration::minutes(30);
        let iso = ts
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        let mut spec = WidgetSpec::new("1", "block-reset-timer");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics {
                resets_at: Some(iso),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = BlockResetTimer.render(&spec, &ctx);
        assert!(!spans[0].text.starts_with("Block reset:"));
    }
}
