//! `flex-separator` — layout-marker widget that expands to fill remaining
//! terminal width. Port of upstream's `flex-separator` manifest entry (see
//! `sirmalloc/ccstatusline/src/utils/renderer.ts` `FLEX_SENTINEL` handling).
//!
//! Unlike normal widgets, this doesn't produce visible text of its own.
//! It emits a single zero-width sentinel span with `flex_hint = true`;
//! the render pipeline's `flex::apply` pass then rewrites the sentinel's
//! `text` to N spaces after all other widgets on the line have rendered,
//! distributing remaining width evenly across all flex slots on the line.
//!
//! When `ctx.terminal_width` is `None` (glassline invoked without a TTY
//! probe) or powerline mode is enabled, the sentinel stays empty and the
//! widget effectively renders nothing — matching upstream's degradation.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(FlexSeparator)
}

pub struct FlexSeparator;

impl Widget for FlexSeparator {
    fn id(&self) -> &'static str {
        "flex-separator"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    fn render(&self, _spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        vec![StyledSpan {
            text: String::new(),
            flex_hint: true,
            ..StyledSpan::default()
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_single_sentinel_span() {
        let spans = FlexSeparator.render(
            &WidgetSpec::new("1", "flex-separator"),
            &RenderContext::default(),
        );
        assert_eq!(spans.len(), 1);
        assert!(spans[0].text.is_empty());
        assert!(spans[0].flex_hint);
    }

    #[test]
    fn sentinel_carries_no_other_style() {
        let spans = FlexSeparator.render(
            &WidgetSpec::new("1", "flex-separator"),
            &RenderContext::default(),
        );
        assert!(!spans[0].bold);
        assert!(!spans[0].dim);
        assert!(!spans[0].italic);
        assert!(!spans[0].underline);
        assert!(!spans[0].gradient_hint);
    }
}
