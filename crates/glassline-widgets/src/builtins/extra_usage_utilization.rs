//! `extra-usage-utilization` — pre-computed percentage of the extra
//! credit budget consumed. Port of upstream `ExtraUsageUtilization.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ExtraUsageUtilization)
}

pub struct ExtraUsageUtilization;

impl Widget for ExtraUsageUtilization {
    fn id(&self) -> &'static str {
        "extra-usage-utilization"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::USAGE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("yellow")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(u) = ctx.usage_data.as_ref() else {
            return Vec::new();
        };
        let Some(util) = u.extra_usage_utilization else {
            return Vec::new();
        };
        let clamped = util.clamp(0.0, 100.0);
        styled(
            spec,
            labeled_or_raw(spec, "Extra: ", &format!("{clamped:.0}%")),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::RenderUsageData;

    fn ctx(util: Option<f64>) -> RenderContext {
        RenderContext {
            usage_data: Some(RenderUsageData {
                extra_usage_utilization: util,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labels_percent() {
        let spans = ExtraUsageUtilization.render(
            &WidgetSpec::new("1", "extra-usage-utilization"),
            &ctx(Some(42.7)),
        );
        assert_eq!(spans[0].text, "Extra: 43%");
    }

    #[test]
    fn clamps_over_100() {
        let spans = ExtraUsageUtilization.render(
            &WidgetSpec::new("1", "extra-usage-utilization"),
            &ctx(Some(120.0)),
        );
        assert_eq!(spans[0].text, "Extra: 100%");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "extra-usage-utilization");
        spec.raw_value = Some(true);
        let spans = ExtraUsageUtilization.render(&spec, &ctx(Some(15.0)));
        assert_eq!(spans[0].text, "15%");
    }

    #[test]
    fn empty_when_absent() {
        let spans = ExtraUsageUtilization
            .render(&WidgetSpec::new("1", "extra-usage-utilization"), &ctx(None));
        assert!(spans.is_empty());
    }
}
