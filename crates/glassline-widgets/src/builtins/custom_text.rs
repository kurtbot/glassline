//! `custom-text` — render whatever the user set in `spec.custom_text`.
//!
//! Port of `src/widgets/CustomText.tsx` from ccstatusline. The TS widget also
//! supports token substitution (`{session_id}`, `{model}`, ...); we ship a
//! minimal subset in the vertical slice and cover the full grammar in
//! P1 T-1.8..T-1.22 alongside the other MVP widgets.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

/// Widget factory used by the registry.
pub fn factory() -> Box<dyn Widget> {
    Box::new(CustomText)
}

/// Zero-sized widget — configuration lives on the spec.
pub struct CustomText;

impl Widget for CustomText {
    fn id(&self) -> &'static str {
        "custom-text"
    }

    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let template = spec.custom_text.as_deref().unwrap_or("");
        let text = expand_placeholders(template, ctx);
        styled(spec, text)
    }
}

/// Vertical-slice placeholder expander. Recognises `{session_id}`, `{model}`,
/// `{cwd}` — enough to prove data flows from stdin through to the rendered
/// output. Full grammar port is deferred to P1 T-1.8+.
fn expand_placeholders(template: &str, ctx: &RenderContext) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        let mut name = String::new();
        let mut closed = false;
        for inner in chars.by_ref() {
            if inner == '}' {
                closed = true;
                break;
            }
            name.push(inner);
        }
        if !closed {
            out.push('{');
            out.push_str(&name);
            continue;
        }
        match name.as_str() {
            "session_id" => {
                if let Some(id) = ctx.data.as_ref().and_then(|d| d.session_id.as_ref()) {
                    out.push_str(id);
                }
            }
            "cwd" => {
                if let Some(cwd) = ctx.data.as_ref().and_then(|d| d.cwd.as_ref()) {
                    out.push_str(cwd);
                }
            }
            "model" => {
                if let Some(model) = ctx.data.as_ref().and_then(|d| d.model.as_ref()) {
                    match model {
                        glassline_core::status_json::ModelInfo::Name(s) => out.push_str(s),
                        glassline_core::status_json::ModelInfo::Full {
                            display_name,
                            id: model_id,
                        } => {
                            if let Some(s) = display_name.as_deref().or(model_id.as_deref()) {
                                out.push_str(s);
                            }
                        }
                    }
                }
            }
            _ => {
                // Unknown placeholder → passthrough literal (TS parity).
                out.push('{');
                out.push_str(&name);
                out.push('}');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{color::Color, status_json::StatusJson};

    fn ctx_with_session(session_id: &str) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                session_id: Some(session_id.into()),
                ..StatusJson::default()
            }),
            ..RenderContext::default()
        }
    }

    #[test]
    fn empty_template_renders_nothing() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some(String::new());
        let out = widget.render(&spec, &RenderContext::default());
        assert!(out.is_empty());
    }

    #[test]
    fn literal_template_renders_as_single_span() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some("hello glassline".into());
        let out = widget.render(&spec, &RenderContext::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "hello glassline");
    }

    #[test]
    fn session_placeholder_expands() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some("s:{session_id}".into());
        let out = widget.render(&spec, &ctx_with_session("abc"));
        assert_eq!(out[0].text, "s:abc");
    }

    #[test]
    fn model_placeholder_prefers_display_name_over_id() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some("[{model}]".into());
        let ctx = RenderContext {
            data: Some(StatusJson {
                model: Some(glassline_core::status_json::ModelInfo::Full {
                    id: Some("claude-opus-4-7".into()),
                    display_name: Some("Opus 4.7".into()),
                }),
                ..StatusJson::default()
            }),
            ..RenderContext::default()
        };
        let out = widget.render(&spec, &ctx);
        assert_eq!(out[0].text, "[Opus 4.7]");
    }

    #[test]
    fn unknown_placeholder_passes_through() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some("[{future_field}]".into());
        let out = widget.render(&spec, &RenderContext::default());
        assert_eq!(out[0].text, "[{future_field}]");
    }

    #[test]
    fn unclosed_placeholder_passes_through() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some("hello {world".into());
        let out = widget.render(&spec, &RenderContext::default());
        assert_eq!(out[0].text, "hello {world");
    }

    #[test]
    fn color_becomes_named_fg() {
        let widget = CustomText;
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some("x".into());
        spec.color = Some("green".into());
        let out = widget.render(&spec, &RenderContext::default());
        assert!(matches!(out[0].fg, Color::Named(ref n) if n == "green"));
    }

    #[test]
    fn widget_id_is_stable() {
        let w = factory();
        assert_eq!(w.id(), "custom-text");
    }

    #[test]
    fn widget_requires_nothing() {
        let w = factory();
        assert_eq!(w.requirements(), WidgetRequirements::NONE);
    }
}
