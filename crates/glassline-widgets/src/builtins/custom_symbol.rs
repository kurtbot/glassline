//! `custom-symbol` — renders `spec.custom_symbol` verbatim. Port of
//! upstream `CustomSymbol.tsx`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(CustomSymbol)
}

pub struct CustomSymbol;

impl Widget for CustomSymbol {
    fn id(&self) -> &'static str {
        "custom-symbol"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    fn render(&self, spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(s) = spec.custom_symbol.as_deref().filter(|s| !s.is_empty()) else {
            return Vec::new();
        };
        styled(spec, s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_configured_symbol() {
        let mut spec = WidgetSpec::new("1", "custom-symbol");
        spec.custom_symbol = Some("★".to_string());
        let spans = CustomSymbol.render(&spec, &RenderContext::default());
        assert_eq!(spans[0].text, "★");
    }

    #[test]
    fn empty_when_symbol_absent() {
        let spec = WidgetSpec::new("1", "custom-symbol");
        let spans = CustomSymbol.render(&spec, &RenderContext::default());
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_symbol_is_empty_string() {
        let mut spec = WidgetSpec::new("1", "custom-symbol");
        spec.custom_symbol = Some("".to_string());
        let spans = CustomSymbol.render(&spec, &RenderContext::default());
        assert!(spans.is_empty());
    }
}
