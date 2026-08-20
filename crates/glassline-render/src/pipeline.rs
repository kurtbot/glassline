//! End-to-end render pipeline.
//!
//! P1 vertical slice: for each `line` in `settings.lines`, resolve each
//! `WidgetSpec` to a [`Widget`], call `render`, and collect the returned
//! [`StyledSpan`]s. The ANSI writer produces one output line per non-empty
//! line of spans.
//!
//! Not yet ported: powerline separators, gradients, flex, per-widget max
//! widths, separator advance state, hyperlinks. Those land in T-1.23+.

use glassline_core::{
    render_context::RenderContext,
    settings::Settings,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};
use glassline_widgets::resolve;
use thiserror::Error;

use crate::ansi::spans_to_string;

/// Compute the union of every visible widget's data requirements.
///
/// Used by the render binary to gate expensive I/O (transcript parsing,
/// git shell-outs, usage HTTP) on whether any widget on the visible lines
/// actually needs the data. Mirror of TS's `hasSpeedItems` /
/// `hasCompactionWidget` / `hasSessionClock` scan in `ccstatusline.ts`.
#[must_use]
pub fn compute_requirements(settings: &Settings) -> WidgetRequirements {
    let mut acc = WidgetRequirements::NONE;
    for line in &settings.lines {
        for spec in line {
            if spec.is_external() {
                // External widgets self-report requirements over the wire
                // protocol (design §4.11). For the P1 slice we conservatively
                // request TRANSCRIPT so ext widgets can read tokens.
                acc |= WidgetRequirements::TRANSCRIPT;
                continue;
            }
            if let Some(widget) = resolve(&spec.kind) {
                acc |= widget.requirements();
            }
        }
    }
    acc
}

/// Assemble the full multi-line status output using a prebuilt context.
///
/// The caller is responsible for populating [`RenderContext`] with any data
/// the widget requirements ([`compute_requirements`]) reported — transcript
/// metrics, git snapshots, usage data. The render side just walks lines and
/// dispatches to widgets.
///
/// Returns the LF-joined output ready to write to stdout. Empty lines (no
/// widget produced any span) are dropped, mirroring TS behaviour.
pub fn render_to_string(
    base_ctx: RenderContext,
    settings: &Settings,
) -> Result<String, PipelineError> {
    let mut out_lines: Vec<String> = Vec::new();
    for (line_index, line) in settings.lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let ctx = RenderContext {
            line_index,
            ..base_ctx.clone()
        };
        let mut line_spans: Vec<StyledSpan> = Vec::new();
        for spec in line {
            if spec.is_external() {
                // P2 wires ext:* through glassline-ext. Until then, emit
                // a visible placeholder so the pipe survives an external
                // widget in the config without dropping the whole line.
                line_spans.push(dim_placeholder(&spec.kind));
                continue;
            }
            let Some(widget) = resolve(&spec.kind) else {
                // Unknown built-in id. Degrade gracefully — real user
                // configs (default TS layout: model / context-length /
                // git-branch / git-changes) get to render partially even
                // when the P1 slice only ships a few widgets. Missing
                // widgets show up as `[?<id>]` so the user knows what to
                // wait for.
                line_spans.push(dim_placeholder(&spec.kind));
                continue;
            };
            line_spans.extend(render_one(widget.as_ref(), spec, &ctx));
        }
        let rendered = spans_to_string(&line_spans);
        if rendered.is_empty() {
            continue;
        }
        out_lines.push(rendered);
    }
    Ok(out_lines.join("\n"))
}

fn render_one(
    widget: &dyn Widget,
    spec: &glassline_core::settings::WidgetSpec,
    ctx: &RenderContext,
) -> Vec<StyledSpan> {
    let mut spans = widget.render(spec, ctx);
    // Apply per-widget default fg color when the settings entry didn't
    // pin one (TS `getDefaultColor()`). Preserves any color the widget
    // already baked in for individual spans (e.g. an error banner).
    if spec.color.is_none()
        && let Some(default_color) = widget.default_color()
    {
        for span in &mut spans {
            if matches!(span.fg, glassline_core::color::Color::Default) {
                span.fg = glassline_core::color::Color::Named(default_color.to_string());
            }
        }
    }
    // Then let metadata-driven animation / gradient / threshold effects
    // reshape the spans. No-op when spec.metadata carries none of the
    // recognized keys.
    glassline_core::animate::apply(spans, spec, ctx.now_ms)
}

/// Wrap an already-rendered status-line block for Claude Code's UI.
///
/// Applies the two TS ccstatusline tweaks (`ccstatusline.ts:213-217`) that
/// keep our styling visible in Claude Code's status area:
///   1. Prepend `\x1b[0m` to each line so Claude Code's dim style on the
///      status-line row doesn't override our per-widget SGR codes.
///   2. Replace regular spaces with non-breaking spaces so VS Code (and
///      Claude Code's own UI when embedded there) doesn't trim trailing
///      space in a widget label.
#[must_use]
pub fn wrap_for_claude_code(rendered: &str) -> String {
    if rendered.is_empty() {
        return rendered.to_string();
    }
    rendered
        .lines()
        .map(|line| format!("\x1b[0m{}", line.replace(' ', "\u{00A0}")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn dim_placeholder(id: &str) -> StyledSpan {
    StyledSpan {
        text: format!("[?{id}]"),
        dim: true,
        ..StyledSpan::default()
    }
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("invalid stdin JSON: {0}")]
    ParseStatusJson(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{
        settings::{Settings, WidgetSpec},
        status_json::StatusJson,
    };

    fn ctx_with(payload: StatusJson) -> RenderContext {
        RenderContext {
            data: Some(payload),
            ..RenderContext::default()
        }
    }

    fn settings_with_one_custom_text(template: &str) -> Settings {
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.custom_text = Some(template.to_string());
        Settings {
            lines: vec![vec![spec], vec![], vec![]],
            ..Settings::in_memory_defaults()
        }
    }

    #[test]
    fn empty_settings_lines_produce_empty_output() {
        let settings = Settings {
            lines: vec![vec![], vec![], vec![]],
            ..Settings::in_memory_defaults()
        };
        let out = render_to_string(ctx_with(StatusJson::default()), &settings).unwrap();
        assert_eq!(out, "");
    }

    #[test]
    fn single_custom_text_widget_renders_literal() {
        let settings = settings_with_one_custom_text("hello glassline");
        let out = render_to_string(ctx_with(StatusJson::default()), &settings).unwrap();
        assert_eq!(out, "hello glassline");
    }

    #[test]
    fn placeholder_expands_from_payload() {
        let payload = StatusJson {
            session_id: Some("abc-123".into()),
            ..StatusJson::default()
        };
        let settings = settings_with_one_custom_text("s:{session_id}");
        let out = render_to_string(ctx_with(payload), &settings).unwrap();
        assert_eq!(out, "s:abc-123");
    }

    #[test]
    fn unknown_widget_renders_dim_placeholder() {
        let spec = WidgetSpec::new("1", "totally-fake-widget");
        let settings = Settings {
            lines: vec![vec![spec], vec![], vec![]],
            ..Settings::in_memory_defaults()
        };
        let out = render_to_string(ctx_with(StatusJson::default()), &settings).unwrap();
        assert!(
            out.contains("[?totally-fake-widget]"),
            "expected placeholder in {out:?}"
        );
    }

    #[test]
    fn ext_prefix_gets_placeholder_in_slice() {
        let spec = WidgetSpec::new("1", "ext:git-worktrees");
        let settings = Settings {
            lines: vec![vec![spec], vec![], vec![]],
            ..Settings::in_memory_defaults()
        };
        let out = render_to_string(ctx_with(StatusJson::default()), &settings).unwrap();
        assert!(out.contains("[?ext:git-worktrees]"));
    }

    #[test]
    fn known_widget_wins_next_to_unknown() {
        let mut known = WidgetSpec::new("k", "custom-text");
        known.custom_text = Some("hi".into());
        // Pick an ID that hasn't been ported yet — cache widgets (5m/1h TTL
        // display, HIT/COLD state) are a P3 batch, so `cache-timer` stays
        // a reliable placeholder stand-in for now.
        let unknown = WidgetSpec::new("u", "cache-timer");
        let settings = Settings {
            lines: vec![vec![known, unknown], vec![], vec![]],
            ..Settings::in_memory_defaults()
        };
        let out = render_to_string(ctx_with(StatusJson::default()), &settings).unwrap();
        assert!(out.contains("hi"));
        assert!(out.contains("[?cache-timer]"));
    }

    #[test]
    fn multiple_lines_are_lf_joined() {
        let mut s1 = WidgetSpec::new("1", "custom-text");
        s1.custom_text = Some("A".into());
        let mut s2 = WidgetSpec::new("2", "custom-text");
        s2.custom_text = Some("B".into());
        let settings = Settings {
            lines: vec![vec![s1], vec![s2], vec![]],
            ..Settings::in_memory_defaults()
        };
        let out = render_to_string(ctx_with(StatusJson::default()), &settings).unwrap();
        assert_eq!(out, "A\nB");
    }

    #[test]
    fn compute_requirements_unions_across_lines() {
        // custom-text has no requirements; unknown widgets contribute
        // nothing; ext:* widgets tentatively pull in TRANSCRIPT.
        let mut ct = WidgetSpec::new("1", "custom-text");
        ct.custom_text = Some("x".into());
        let ext = WidgetSpec::new("2", "ext:something");
        let settings = Settings {
            lines: vec![vec![ct, ext], vec![], vec![]],
            ..Settings::in_memory_defaults()
        };
        let reqs = compute_requirements(&settings);
        assert!(reqs.contains(WidgetRequirements::TRANSCRIPT));
    }
}
