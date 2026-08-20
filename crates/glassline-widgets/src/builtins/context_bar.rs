//! `context-bar` — a filled/empty block bar showing context-window usage.
//! Port of TS `ContextBar.ts`. Supported display modes (per
//! `metadata.display`):
//!
//! | Mode | Width | Label | Notes |
//! |---|---|---|---|
//! | `progress-short` (default) | 16 | `used/total (pct%)` | filled block bar `[███░░░]` |
//! | `progress` | 32 | `used/total (pct%)` | same, wider |
//! | `slider` | 16 | `used/total (pct%)` | caret bar `[───●───]` |
//! | `slider-only` | 16 | none | same caret, no numeric label |

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{
    context_window_metrics, default_context_window_size, format_tokens, is_raw, styled,
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ContextBar)
}

pub struct ContextBar;

impl Widget for ContextBar {
    fn id(&self) -> &'static str {
        "context-bar"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let metrics = context_window_metrics(ctx.data.as_ref());
        let total = metrics
            .window_size
            .unwrap_or_else(default_context_window_size);
        let used = metrics
            .context_length_tokens
            .or_else(|| ctx.token_metrics.as_ref().map(|m| m.context_length));
        let Some(used) = used else {
            return Vec::new();
        };
        if total == 0 {
            return Vec::new();
        }

        let percent = (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0);
        let mode = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("display"))
            .map(String::as_str)
            .unwrap_or("progress-short");
        let (bar, include_label) = match mode {
            "progress" => (make_progress_bar(percent, 32), true),
            "slider" => (make_slider_bar(percent, 16), true),
            "slider-only" => (make_slider_bar(percent, 16), false),
            // Anything else falls through to the TS default.
            _ => (make_progress_bar(percent, 16), true),
        };

        let display = if include_label {
            format!(
                "{bar} {used_disp}/{total_disp} ({pct}%)",
                used_disp = format_tokens(used, 0),
                total_disp = format_tokens(total, 0),
                pct = percent.round() as u32,
            )
        } else {
            bar
        };
        let text = if is_raw(spec) {
            display
        } else {
            format!("Context: {display}")
        };
        styled(spec, text)
    }
}

/// Filled/empty block bar: `[███░░░]`.
fn make_progress_bar(percent: f64, width: usize) -> String {
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.min(width);
    let empty = width - filled;
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for _ in 0..filled {
        s.push('█');
    }
    for _ in 0..empty {
        s.push('░');
    }
    s.push(']');
    s
}

/// Caret bar: `[───●───]`. Caret position is `percent` of `width`.
fn make_slider_bar(percent: f64, width: usize) -> String {
    let pos = ((percent / 100.0) * width as f64).round() as usize;
    let pos = pos.min(width.saturating_sub(1));
    let mut s = String::with_capacity(width + 2);
    s.push('[');
    for i in 0..width {
        if i == pos {
            s.push('\u{25CF}'); // ●
        } else {
            s.push('\u{2500}'); // ─
        }
    }
    s.push(']');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{
        render_context::TokenMetrics,
        status_json::{ContextWindow, StatusJson},
    };

    #[test]
    fn labeled_progress_short_default() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    context_window_size: Some(200_000.0),
                    used_percentage: Some(25.0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextBar.render(&WidgetSpec::new("1", "context-bar"), &ctx);
        // used_tokens computed from used_percentage since current_usage is
        // absent: 25% of 200k = 50k -> context_length_tokens falls back to
        // used_tokens = 50k
        assert!(spans[0].text.starts_with("Context: ["));
        assert!(spans[0].text.contains("50k/200k"));
        assert!(spans[0].text.contains("(25%)"));
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "context-bar");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 100_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextBar.render(&spec, &ctx);
        assert!(spans[0].text.starts_with("["));
        assert!(spans[0].text.contains("100k/200k"));
        assert!(spans[0].text.contains("(50%)"));
    }

    #[test]
    fn wider_progress_mode() {
        let mut spec = WidgetSpec::new("1", "context-bar");
        spec.metadata = Some(
            [("display".to_string(), "progress".to_string())]
                .into_iter()
                .collect(),
        );
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 100_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextBar.render(&spec, &ctx);
        // `progress` uses 32 blocks — count them post-`[`.
        let text = &spans[0].text;
        let bar_start = text.find('[').unwrap();
        let bar_end = text.find(']').unwrap();
        let bar_chars = text[bar_start + 1..bar_end].chars().count();
        assert_eq!(bar_chars, 32);
    }

    #[test]
    fn slider_mode_renders_caret() {
        let mut spec = WidgetSpec::new("1", "context-bar");
        spec.metadata = Some(
            [("display".to_string(), "slider".to_string())]
                .into_iter()
                .collect(),
        );
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 100_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextBar.render(&spec, &ctx);
        let text = &spans[0].text;
        assert!(text.starts_with("Context: ["));
        assert!(text.contains('\u{25CF}'), "slider caret missing in {text}");
        assert!(text.contains("100k/200k"));
    }

    #[test]
    fn slider_only_drops_numeric_label() {
        let mut spec = WidgetSpec::new("1", "context-bar");
        spec.metadata = Some(
            [("display".to_string(), "slider-only".to_string())]
                .into_iter()
                .collect(),
        );
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 100_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextBar.render(&spec, &ctx);
        let text = &spans[0].text;
        assert!(text.contains('\u{25CF}'), "slider caret missing in {text}");
        assert!(
            !text.contains('/'),
            "slider-only should not include the numeric label: {text}"
        );
        assert!(
            !text.contains('%'),
            "slider-only should not include the pct label: {text}"
        );
    }

    #[test]
    fn empty_when_no_context_available() {
        let spans = ContextBar.render(
            &WidgetSpec::new("1", "context-bar"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
