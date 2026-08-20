//! `separator` — renders the settings-configured default separator (or
//! ` | ` as a fallback) between widgets. Port of TS `Separator.tsx`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(Separator)
}

pub struct Separator;

impl Widget for Separator {
    fn id(&self) -> &'static str {
        "separator"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    fn render(&self, spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        // Precedence per TS: spec.character > settings.default_separator > " | ".
        // The renderer will inject settings-level defaults; here we honour the
        // per-widget override only. Full precedence lands with T-1.23 renderer.
        let text = spec.character.clone().unwrap_or_else(|| " | ".to_string());
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_separator_is_bar_padded() {
        let spans = Separator.render(
            &WidgetSpec::new("1", "separator"),
            &RenderContext::default(),
        );
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, " | ");
    }

    #[test]
    fn per_widget_character_override() {
        let mut spec = WidgetSpec::new("1", "separator");
        spec.character = Some(" · ".into());
        let spans = Separator.render(&spec, &RenderContext::default());
        assert_eq!(spans[0].text, " · ");
    }
}
