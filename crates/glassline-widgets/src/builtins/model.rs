//! `model` — displays the current Claude model name.
//! Port of TS `Model.tsx` (render path).

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    status_json::ModelInfo,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(Model)
}

pub struct Model;

impl Widget for Model {
    fn id(&self) -> &'static str {
        "model"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(model) = ctx.data.as_ref().and_then(|d| d.model.as_ref()) else {
            return Vec::new();
        };
        let name = match model {
            ModelInfo::Name(s) => s.clone(),
            ModelInfo::Full { display_name, id } => display_name
                .clone()
                .or_else(|| id.clone())
                .unwrap_or_default(),
        };
        if name.is_empty() {
            return Vec::new();
        }
        styled(spec, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::StatusJson;

    fn ctx(model: Option<ModelInfo>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                model,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn renders_bare_string_model() {
        let spans = Model.render(
            &WidgetSpec::new("1", "model"),
            &ctx(Some(ModelInfo::Name("claude-opus-4-7".into()))),
        );
        assert_eq!(spans[0].text, "claude-opus-4-7");
    }

    #[test]
    fn prefers_display_name_over_id() {
        let spans = Model.render(
            &WidgetSpec::new("1", "model"),
            &ctx(Some(ModelInfo::Full {
                id: Some("claude-opus-4-7".into()),
                display_name: Some("Opus 4.7".into()),
            })),
        );
        assert_eq!(spans[0].text, "Opus 4.7");
    }

    #[test]
    fn empty_when_model_absent() {
        let spans = Model.render(&WidgetSpec::new("1", "model"), &RenderContext::default());
        assert!(spans.is_empty());
    }
}
