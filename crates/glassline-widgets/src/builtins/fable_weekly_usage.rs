//! `fable-weekly-usage` — Fable model's weekly-usage percentage. Port of
//! upstream `FableWeeklyUsage.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(FableWeeklyUsage)
}

pub struct FableWeeklyUsage;

impl Widget for FableWeeklyUsage {
    fn id(&self) -> &'static str {
        "fable-weekly-usage"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::USAGE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightMagenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(u) = ctx.usage_data.as_ref() else {
            return Vec::new();
        };
        let Some(pct) = u.fable_usage else {
            return Vec::new();
        };
        styled(
            spec,
            labeled_or_raw(spec, "Weekly Fable: ", &format!("{pct:.0}%")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::RenderUsageData;

    fn ctx(pct: Option<f64>) -> RenderContext {
        RenderContext {
            usage_data: Some(RenderUsageData {
                fable_usage: pct,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labels_percent() {
        let spans = FableWeeklyUsage.render(
            &WidgetSpec::new("1", "fable-weekly-usage"),
            &ctx(Some(12.7)),
        );
        assert_eq!(spans[0].text, "Weekly Fable: 13%");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "fable-weekly-usage");
        spec.raw_value = Some(true);
        let spans = FableWeeklyUsage.render(&spec, &ctx(Some(80.0)));
        assert_eq!(spans[0].text, "80%");
    }

    #[test]
    fn empty_when_absent() {
        let spans =
            FableWeeklyUsage.render(&WidgetSpec::new("1", "fable-weekly-usage"), &ctx(None));
        assert!(spans.is_empty());
    }
}
