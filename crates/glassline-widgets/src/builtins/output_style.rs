//! `output-style` — Claude Code's active output-style name. Port of
//! upstream `OutputStyle.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(OutputStyleWidget)
}

pub struct OutputStyleWidget;

impl Widget for OutputStyleWidget {
    fn id(&self) -> &'static str {
        "output-style"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(name) = ctx
            .data
            .as_ref()
            .and_then(|d| d.output_style.as_ref())
            .and_then(|o| o.name.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        styled(spec, labeled_or_raw(spec, "Style: ", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{OutputStyle, StatusJson};

    fn ctx(style: Option<&str>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                output_style: Some(OutputStyle {
                    name: style.map(String::from),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn labels_output_style() {
        let spans = OutputStyleWidget.render(
            &WidgetSpec::new("1", "output-style"),
            &ctx(Some("Explanatory")),
        );
        assert_eq!(spans[0].text, "Style: Explanatory");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "output-style");
        spec.raw_value = Some(true);
        let spans = OutputStyleWidget.render(&spec, &ctx(Some("Learning")));
        assert_eq!(spans[0].text, "Learning");
    }

    #[test]
    fn empty_when_name_absent() {
        let spans = OutputStyleWidget.render(&WidgetSpec::new("1", "output-style"), &ctx(None));
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_name_is_empty_string() {
        let spans = OutputStyleWidget.render(&WidgetSpec::new("1", "output-style"), &ctx(Some("")));
        assert!(spans.is_empty());
    }
}
