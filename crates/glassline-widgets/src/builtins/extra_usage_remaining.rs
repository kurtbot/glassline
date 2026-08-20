//! `extra-usage-remaining` — remaining extra-credit budget = `limit -
//! used`. Port of upstream `ExtraUsageRemaining.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ExtraUsageRemaining)
}

pub struct ExtraUsageRemaining;

impl Widget for ExtraUsageRemaining {
    fn id(&self) -> &'static str {
        "extra-usage-remaining"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::USAGE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(u) = ctx.usage_data.as_ref() else {
            return Vec::new();
        };
        let (Some(limit), Some(used)) = (u.extra_usage_limit, u.extra_usage_used) else {
            return Vec::new();
        };
        let remaining = (limit - used).max(0.0);
        let currency = u.extra_usage_currency.as_deref().unwrap_or("USD");
        let symbol = match currency {
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "JPY" => "¥",
            _ => "$",
        };
        let formatted = format!("{symbol}{remaining:.2}");
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("Extra left: {formatted}")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::RenderUsageData;

    fn ctx(limit: Option<f64>, used: Option<f64>) -> RenderContext {
        RenderContext {
            usage_data: Some(RenderUsageData {
                extra_usage_limit: limit,
                extra_usage_used: used,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn computes_limit_minus_used() {
        let spans = ExtraUsageRemaining.render(
            &WidgetSpec::new("1", "extra-usage-remaining"),
            &ctx(Some(50.0), Some(12.34)),
        );
        assert_eq!(spans[0].text, "Extra left: $37.66");
    }

    #[test]
    fn floor_at_zero_when_overspent() {
        let spans = ExtraUsageRemaining.render(
            &WidgetSpec::new("1", "extra-usage-remaining"),
            &ctx(Some(10.0), Some(15.0)),
        );
        assert_eq!(spans[0].text, "Extra left: $0.00");
    }

    #[test]
    fn empty_without_limit() {
        let spans = ExtraUsageRemaining.render(
            &WidgetSpec::new("1", "extra-usage-remaining"),
            &ctx(None, Some(5.0)),
        );
        assert!(spans.is_empty());
    }
}
