//! `custom-text` — render whatever the user set in `spec.custom_text`.
//!
//! Port of `src/widgets/CustomText.tsx` from ccstatusline, including
//! the full placeholder grammar (Tier J of
//! [[widget_parity_design_v1.1]] §4.10).
//!
//! # Recognised placeholders
//!
//! | Token | Source |
//! |---|---|
//! | `{session_id}` | `StatusJson.session_id` |
//! | `{session_name}` | `StatusJson.session_name` |
//! | `{cwd}` | `StatusJson.cwd` |
//! | `{transcript_path}` | `StatusJson.transcript_path` |
//! | `{version}` | `StatusJson.version` |
//! | `{output_style}` | `StatusJson.output_style.name` |
//! | `{model}` | Whichever of `display_name` / `id` is present |
//! | `{model_id}` | `ModelInfo::Full.id` (empty for bare-string form) |
//! | `{model_display}` | `ModelInfo::Full.display_name` (or the bare string) |
//! | `{cost}` | `Cost.total_cost_usd` formatted as `$X.XX` |
//! | `{elapsed}` | `Cost.total_duration_ms` via `format_duration_ms` |
//! | `{tokens}` | `TokenMetrics::total()` via `format_tokens(_,1)` |
//! | `{context_percent}` | `context_window.used_percentage` as `X.X%` |
//! | `{terminal_width}` | `ctx.terminal_width` |
//! | `{git_branch}` | Shell-out via `crate::git::get_git_branch` |
//!
//! Unrecognised placeholders pass through as literal `{...}` (TS parity).

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    status_json::ModelInfo,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{DurationFormat, format_duration_ms, format_tokens, styled};

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
        if let Some(value) = lookup(&name, ctx) {
            out.push_str(&value);
        } else {
            // Unknown placeholder → passthrough literal (TS parity).
            out.push('{');
            out.push_str(&name);
            out.push('}');
        }
    }
    out
}

/// Resolve a placeholder name to its expanded text. `None` for unknown
/// names (caller re-emits the `{name}` literal); `Some("".to_string())`
/// for known names whose data is absent (empty expansion).
fn lookup(name: &str, ctx: &RenderContext) -> Option<String> {
    let data = ctx.data.as_ref();
    match name {
        "session_id" => Some(data.and_then(|d| d.session_id.clone()).unwrap_or_default()),
        "session_name" => Some(
            data.and_then(|d| d.session_name.clone())
                .unwrap_or_default(),
        ),
        "cwd" => Some(data.and_then(|d| d.cwd.clone()).unwrap_or_default()),
        "transcript_path" => Some(
            data.and_then(|d| d.transcript_path.clone())
                .unwrap_or_default(),
        ),
        "version" => Some(data.and_then(|d| d.version.clone()).unwrap_or_default()),
        "output_style" => Some(
            data.and_then(|d| d.output_style.as_ref())
                .and_then(|o| o.name.clone())
                .unwrap_or_default(),
        ),
        "model" => Some(resolve_model(data, ModelField::Display)),
        "model_id" => Some(resolve_model(data, ModelField::Id)),
        "model_display" => Some(resolve_model(data, ModelField::Display)),
        "cost" => Some(
            data.and_then(|d| d.cost.as_ref())
                .and_then(|c| c.total_cost_usd)
                .map(|v| format!("${v:.2}"))
                .unwrap_or_default(),
        ),
        "elapsed" => Some(
            data.and_then(|d| d.cost.as_ref())
                .and_then(|c| c.total_duration_ms)
                .filter(|v| v.is_finite() && *v >= 0.0)
                .map(|ms| {
                    format_duration_ms(
                        ms as u64,
                        DurationFormat {
                            compact: false,
                            use_days: false,
                            less_than_min: true,
                            ..DurationFormat::default()
                        },
                    )
                })
                .unwrap_or_default(),
        ),
        "tokens" => Some(
            ctx.token_metrics
                .as_ref()
                .map(|m| format_tokens(m.total(), 1))
                .unwrap_or_default(),
        ),
        "context_percent" => Some(
            data.and_then(|d| d.context_window.as_ref())
                .and_then(|cw| cw.used_percentage)
                .map(|p| format!("{p:.1}%"))
                .unwrap_or_default(),
        ),
        "terminal_width" => Some(
            ctx.terminal_width
                .map(|w| w.to_string())
                .unwrap_or_default(),
        ),
        "git_branch" => Some(crate::git::get_git_branch(ctx).unwrap_or_default()),
        _ => None,
    }
}

enum ModelField {
    Display,
    Id,
}

fn resolve_model(
    data: Option<&glassline_core::status_json::StatusJson>,
    field: ModelField,
) -> String {
    let Some(model) = data.and_then(|d| d.model.as_ref()) else {
        return String::new();
    };
    match (model, field) {
        (ModelInfo::Name(s), _) => s.clone(),
        (ModelInfo::Full { display_name, id }, ModelField::Display) => display_name
            .clone()
            .or_else(|| id.clone())
            .unwrap_or_default(),
        (ModelInfo::Full { id, .. }, ModelField::Id) => id.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{
        color::Color,
        render_context::TokenMetrics,
        status_json::{ContextWindow, Cost, OutputStyle, StatusJson},
    };

    fn spec_with(template: &str) -> WidgetSpec {
        let mut s = WidgetSpec::new("1", "custom-text");
        s.custom_text = Some(template.to_string());
        s
    }

    #[test]
    fn empty_template_renders_nothing() {
        let out = CustomText.render(&spec_with(""), &RenderContext::default());
        assert!(out.is_empty());
    }

    #[test]
    fn literal_template_renders_as_single_span() {
        let out = CustomText.render(&spec_with("hello glassline"), &RenderContext::default());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "hello glassline");
    }

    #[test]
    fn session_id_expands() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                session_id: Some("abc".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let out = CustomText.render(&spec_with("s:{session_id}"), &ctx);
        assert_eq!(out[0].text, "s:abc");
    }

    #[test]
    fn model_display_prefers_display_over_id() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                model: Some(ModelInfo::Full {
                    id: Some("claude-opus-4-7".into()),
                    display_name: Some("Opus 4.7".into()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{model}"), &ctx)[0].text,
            "Opus 4.7"
        );
        assert_eq!(
            CustomText.render(&spec_with("{model_display}"), &ctx)[0].text,
            "Opus 4.7"
        );
        assert_eq!(
            CustomText.render(&spec_with("{model_id}"), &ctx)[0].text,
            "claude-opus-4-7"
        );
    }

    #[test]
    fn cost_formatted_as_dollars() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                cost: Some(Cost {
                    total_cost_usd: Some(2.45),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{cost}"), &ctx)[0].text,
            "$2.45"
        );
    }

    #[test]
    fn elapsed_formatted_via_duration_helper() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                cost: Some(Cost {
                    total_duration_ms: Some(3_720_000.0), // 62 min
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{elapsed}"), &ctx)[0].text,
            "1hr 2m"
        );
    }

    #[test]
    fn tokens_from_metrics() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                input: 10_000,
                output: 5_000,
                cache_read: 20_000,
                cache_creation: 0,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{tokens}"), &ctx)[0].text,
            "35.0k"
        );
    }

    #[test]
    fn context_percent_expands() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    used_percentage: Some(42.7),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{context_percent}"), &ctx)[0].text,
            "42.7%"
        );
    }

    #[test]
    fn output_style_expands() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                output_style: Some(OutputStyle {
                    name: Some("Explanatory".into()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{output_style}"), &ctx)[0].text,
            "Explanatory"
        );
    }

    #[test]
    fn terminal_width_expands() {
        let ctx = RenderContext {
            terminal_width: Some(120),
            ..Default::default()
        };
        assert_eq!(
            CustomText.render(&spec_with("{terminal_width}"), &ctx)[0].text,
            "120"
        );
    }

    #[test]
    fn absent_data_expands_to_empty_string() {
        // Known placeholder + missing data → empty (no `{...}` literal
        // leftover), matching TS behavior.
        assert_eq!(
            CustomText.render(&spec_with("[{cost}]"), &RenderContext::default())[0].text,
            "[]"
        );
    }

    #[test]
    fn unknown_placeholder_passes_through() {
        assert_eq!(
            CustomText.render(&spec_with("[{future_field}]"), &RenderContext::default())[0].text,
            "[{future_field}]"
        );
    }

    #[test]
    fn unclosed_placeholder_passes_through() {
        assert_eq!(
            CustomText.render(&spec_with("hello {world"), &RenderContext::default())[0].text,
            "hello {world"
        );
    }

    #[test]
    fn color_becomes_named_fg() {
        let mut spec = spec_with("x");
        spec.color = Some("green".into());
        let out = CustomText.render(&spec, &RenderContext::default());
        assert!(matches!(out[0].fg, Color::Named(ref n) if n == "green"));
    }

    #[test]
    fn widget_id_is_stable() {
        assert_eq!(factory().id(), "custom-text");
    }

    #[test]
    fn widget_requires_nothing() {
        assert_eq!(factory().requirements(), WidgetRequirements::NONE);
    }
}
