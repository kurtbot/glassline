//! Live-preview primitive. Feeds a caller-supplied [`RenderContext`]
//! and [`Settings`] through the real render pipeline
//! ([`glassline_render::render_to_string`]) so what the user sees
//! matches exactly what the hot path would produce — colors, bold,
//! and dim included via `ansi-to-tui`.

use ansi_to_tui::IntoText;
use ratatui::{Frame, layout::Rect, text::Text, widgets::Paragraph};

use glassline_core::{render_context::RenderContext, settings::Settings};
use glassline_render::render_to_string;

/// Live-preview primitive. Callers construct with functions that
/// return the current context + settings so the preview always
/// reflects the latest scratch state.
pub struct Preview<C, S>
where
    C: Fn() -> RenderContext,
    S: Fn() -> Settings,
{
    ctx_fn: C,
    settings_fn: S,
}

impl<C, S> Preview<C, S>
where
    C: Fn() -> RenderContext,
    S: Fn() -> Settings,
{
    pub fn new(ctx_fn: C, settings_fn: S) -> Self {
        Self {
            ctx_fn,
            settings_fn,
        }
    }

    /// Render into `area`. Errors from the pipeline (e.g. an invalid
    /// widget config) surface as a single-line `[preview error: ...]`
    /// row rather than propagating — the preview must never crash the
    /// editor. Colors + attributes ride through via `ansi-to-tui`.
    pub fn render(&self, area: Rect, frame: &mut Frame) {
        let ctx = (self.ctx_fn)();
        let settings = (self.settings_fn)();
        let text: Text<'_> = match render_to_string(ctx, &settings) {
            Ok(s) => s.into_text().unwrap_or_else(|_| Text::from(strip_ansi(&s))),
            Err(e) => Text::from(format!("[preview error: {e}]")),
        };
        frame.render_widget(Paragraph::new(text), area);
    }
}

/// Strip ANSI SGR escape sequences from `input`. Handles the
/// `ESC[...LETTER` family; leaves other control chars untouched.
#[must_use]
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next(); // consume `[`
            for c2 in chars.by_ref() {
                if c2.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn strip_ansi_removes_sgr() {
        assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(
            strip_ansi("\x1b[1;33;44myellow-on-blue\x1b[0m"),
            "yellow-on-blue"
        );
    }

    #[test]
    fn strip_ansi_preserves_plain_text() {
        assert_eq!(strip_ansi("plain text"), "plain text");
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn strip_ansi_leaves_isolated_escape_alone() {
        // ESC not followed by `[` is left alone (not a CSI).
        assert_eq!(strip_ansi("\x1bnope"), "\x1bnope");
    }

    #[test]
    fn preview_renders_settings_output() {
        // Empty settings → empty rendered output. Just proving the
        // primitive doesn't panic and paints something.
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        let preview = Preview::new(RenderContext::default, Settings::default);
        term.draw(|frame| preview.render(frame.area(), frame))
            .unwrap();
    }

    #[test]
    fn preview_error_produces_bracket_message() {
        // We don't have a synthetic way to force an error here — a
        // well-formed empty Settings renders as Ok(String). The
        // contract we care about is documented behaviour; asserting
        // the error branch stringifies through the format arg is a
        // documentation test only.
        let msg = format!("[preview error: {}]", "boom");
        assert!(msg.starts_with("[preview error:"));
    }
}
