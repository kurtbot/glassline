//! `block-timer` — elapsed time in the current 5-hour billing block.
//! Port of upstream `BlockTimer.ts`.
//!
//! Reads `ctx.block_metrics.started_at` (RFC3339). The render binary is
//! responsible for populating `block_metrics`; if it's absent (or the
//! timestamp is malformed) the widget renders nothing.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{DurationFormat, duration_since_iso_ms, format_duration_ms, is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(BlockTimer)
}

pub struct BlockTimer;

impl Widget for BlockTimer {
    fn id(&self) -> &'static str {
        "block-timer"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::BLOCK
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(started_at) = ctx
            .block_metrics
            .as_ref()
            .and_then(|b| b.started_at.as_deref())
        else {
            return Vec::new();
        };
        let Some(ms) = duration_since_iso_ms(started_at) else {
            return Vec::new();
        };
        let base = DurationFormat {
            compact: false,
            use_days: false,
            less_than_min: true,
            show_seconds: false,
        };
        let fmt = DurationFormat::from_metadata(base, spec);
        let formatted = format_duration_ms(ms, fmt);
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("Block: {formatted}")
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
        let spans = BlockTimer.render(
            &WidgetSpec::new("1", "block-timer"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_started_at_absent() {
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics::default()),
            ..Default::default()
        };
        let spans = BlockTimer.render(&WidgetSpec::new("1", "block-timer"), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_started_at_unparseable() {
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics {
                started_at: Some("not-a-date".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = BlockTimer.render(&WidgetSpec::new("1", "block-timer"), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn renders_labeled_when_started_at_is_recent() {
        // Started 1 hour ago (roughly — actual now-1h; since format has
        // minute-precision the test only asserts the label + suffix, not
        // exact numeric).
        let now = time::OffsetDateTime::now_utc();
        let ts = now - time::Duration::hours(1);
        let started = ts
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap();
        let ctx = RenderContext {
            block_metrics: Some(BlockMetrics {
                started_at: Some(started),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = BlockTimer.render(&WidgetSpec::new("1", "block-timer"), &ctx);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].text.starts_with("Block: "));
        // Elapsed ≥ 1hr; expect "1hr" somewhere.
        assert!(spans[0].text.contains("1hr"), "got {}", spans[0].text);
    }
}
