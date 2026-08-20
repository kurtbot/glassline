//! `terminal-width` — the width of the terminal in columns, as detected
//! by the render binary and passed via `ctx.terminal_width`. Port of
//! upstream `TerminalWidth.ts`.
//!
//! Rendering is trivial — reads the pre-populated field. The render
//! binary is responsible for probing the terminal (COLUMNS env var,
//! ioctl TIOCGWINSZ, `tput cols`, etc.). When `terminal_width` is
//! absent, the widget renders nothing.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(TerminalWidth)
}

pub struct TerminalWidth;

impl Widget for TerminalWidth {
    fn id(&self) -> &'static str {
        "terminal-width"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(w) = ctx.terminal_width else {
            return Vec::new();
        };
        styled(spec, labeled_or_raw(spec, "Term: ", &w.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_width() {
        let ctx = RenderContext {
            terminal_width: Some(120),
            ..Default::default()
        };
        let spans = TerminalWidth.render(&WidgetSpec::new("1", "terminal-width"), &ctx);
        assert_eq!(spans[0].text, "Term: 120");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "terminal-width");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            terminal_width: Some(80),
            ..Default::default()
        };
        let spans = TerminalWidth.render(&spec, &ctx);
        assert_eq!(spans[0].text, "80");
    }

    #[test]
    fn empty_when_absent() {
        let spans = TerminalWidth.render(
            &WidgetSpec::new("1", "terminal-width"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
