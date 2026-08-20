//! `compaction-counter` — number of compactions in the current session.
//! Port of upstream `CompactionCounter.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(CompactionCounter)
}

pub struct CompactionCounter;

impl Widget for CompactionCounter {
    fn id(&self) -> &'static str {
        "compaction-counter"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::COMPACTION
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("yellow")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(data) = ctx.compaction_data.as_ref() else {
            return Vec::new();
        };
        let value = data.count.to_string();
        styled(spec, labeled_or_raw(spec, "Compactions: ", &value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::CompactionData;

    fn ctx(count: u64) -> RenderContext {
        RenderContext {
            compaction_data: Some(CompactionData {
                count,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labeled_count() {
        let spans = CompactionCounter.render(&WidgetSpec::new("1", "compaction-counter"), &ctx(3));
        assert_eq!(spans[0].text, "Compactions: 3");
    }

    #[test]
    fn zero_still_renders() {
        // Distinct from "no data" — count=0 is a real value ("no compactions
        // yet"). Widget renders it explicitly.
        let spans = CompactionCounter.render(&WidgetSpec::new("1", "compaction-counter"), &ctx(0));
        assert_eq!(spans[0].text, "Compactions: 0");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "compaction-counter");
        spec.raw_value = Some(true);
        let spans = CompactionCounter.render(&spec, &ctx(5));
        assert_eq!(spans[0].text, "5");
    }

    #[test]
    fn empty_when_no_compaction_data() {
        let spans = CompactionCounter.render(
            &WidgetSpec::new("1", "compaction-counter"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
