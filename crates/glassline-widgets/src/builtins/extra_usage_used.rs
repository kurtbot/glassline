//! `extra-usage-used` — dollars (or configured currency) of extra
//! credit consumed this month. Port of upstream `ExtraUsageUsed.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ExtraUsageUsed)
}

pub struct ExtraUsageUsed;

impl Widget for ExtraUsageUsed {
    fn id(&self) -> &'static str {
        "extra-usage-used"
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
        let Some(used) = u.extra_usage_used else {
            return Vec::new();
        };
        let currency = u.extra_usage_currency.as_deref().unwrap_or("USD");
        let symbol = currency_symbol(currency);
        let formatted = format!("{symbol}{used:.2}");
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("Extra used: {formatted}")
        };
        styled(spec, text)
    }
}

fn currency_symbol(code: &str) -> &'static str {
    match code {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        _ => "$", // unknown → default to $ rather than dropping data
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::RenderUsageData;

    fn ctx(used: Option<f64>, currency: Option<&str>) -> RenderContext {
        RenderContext {
            usage_data: Some(RenderUsageData {
                extra_usage_used: used,
                extra_usage_currency: currency.map(String::from),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labels_dollar_by_default() {
        let spans = ExtraUsageUsed.render(
            &WidgetSpec::new("1", "extra-usage-used"),
            &ctx(Some(12.34), None),
        );
        assert_eq!(spans[0].text, "Extra used: $12.34");
    }

    #[test]
    fn uses_currency_symbol_when_known() {
        let spans = ExtraUsageUsed.render(
            &WidgetSpec::new("1", "extra-usage-used"),
            &ctx(Some(5.0), Some("EUR")),
        );
        assert_eq!(spans[0].text, "Extra used: €5.00");
    }

    #[test]
    fn unknown_currency_falls_back_to_dollar() {
        let spans = ExtraUsageUsed.render(
            &WidgetSpec::new("1", "extra-usage-used"),
            &ctx(Some(1.5), Some("XYZ")),
        );
        assert_eq!(spans[0].text, "Extra used: $1.50");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "extra-usage-used");
        spec.raw_value = Some(true);
        let spans = ExtraUsageUsed.render(&spec, &ctx(Some(2.0), None));
        assert_eq!(spans[0].text, "$2.00");
    }

    #[test]
    fn empty_when_absent() {
        let spans =
            ExtraUsageUsed.render(&WidgetSpec::new("1", "extra-usage-used"), &ctx(None, None));
        assert!(spans.is_empty());
    }
}
