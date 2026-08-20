//! `session-cost` — renders `Cost: $2.45` (or `$2.45` raw) from
//! `StatusJson.cost.total_cost_usd`. Port of TS `SessionCost.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(SessionCost)
}

pub struct SessionCost;

impl Widget for SessionCost {
    fn id(&self) -> &'static str {
        "session-cost"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(cost) = ctx.data.as_ref().and_then(|d| d.cost.as_ref()) else {
            return Vec::new();
        };
        let Some(total) = cost.total_cost_usd else {
            return Vec::new();
        };
        let formatted = format!("${total:.2}");
        styled(spec, labeled_or_raw(spec, "Cost: ", &formatted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{Cost, StatusJson};

    fn ctx(cost: Option<f64>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                cost: Some(Cost {
                    total_cost_usd: cost,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labeled_default() {
        let spans = SessionCost.render(&WidgetSpec::new("1", "session-cost"), &ctx(Some(2.45)));
        assert_eq!(spans[0].text, "Cost: $2.45");
    }

    #[test]
    fn raw_value_drops_label() {
        let mut spec = WidgetSpec::new("1", "session-cost");
        spec.raw_value = Some(true);
        let spans = SessionCost.render(&spec, &ctx(Some(2.45)));
        assert_eq!(spans[0].text, "$2.45");
    }

    #[test]
    fn empty_when_cost_absent() {
        let spans = SessionCost.render(&WidgetSpec::new("1", "session-cost"), &ctx(None));
        assert!(spans.is_empty());
    }

    #[test]
    fn two_decimal_places_regardless_of_input() {
        let spans = SessionCost.render(&WidgetSpec::new("1", "session-cost"), &ctx(Some(7.0)));
        assert_eq!(spans[0].text, "Cost: $7.00");
    }
}
