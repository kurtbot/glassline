//! Single-line text input wrapping [`tui_input::Input`].
//!
//! Handles unicode-width-aware cursor placement, horizontal scroll for
//! text longer than the visible area, and an optional length cap.
//!
//! Screens forward crossterm key events via [`TextInput::handle_event`]
//! and render via [`TextInput::render`]. The primitive also positions
//! the terminal cursor via `Frame::set_cursor_position` when the input
//! is focused — pass `focused = true` to `render` to enable that.

use ratatui::{
    Frame,
    crossterm::event::Event,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph},
};
use tui_input::{Input, backend::crossterm::EventHandler};

/// Single-line text input with visible-hint + optional length cap.
#[derive(Debug, Default)]
pub struct TextInput {
    input: Input,
    hint: String,
    max_len: Option<usize>,
}

impl TextInput {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Preload the input buffer.
    #[must_use]
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.input = Input::new(value.into());
        self
    }

    /// Show `hint` when the buffer is empty. Not stored inside the
    /// buffer; only rendered as placeholder text.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// Cap the buffer at `max_len` characters. Input beyond the cap is
    /// silently discarded by `handle_event`.
    #[must_use]
    pub fn with_max_len(mut self, max_len: usize) -> Self {
        self.max_len = Some(max_len);
        self
    }

    /// The current text.
    #[must_use]
    pub fn value(&self) -> &str {
        self.input.value()
    }

    /// Return the current buffer and reset the input.
    pub fn take(&mut self) -> String {
        self.input.value_and_reset()
    }

    /// Reset the buffer.
    pub fn clear(&mut self) {
        self.input.reset();
    }

    /// Forward an event to the underlying [`Input`]. Returns `true` if
    /// the buffer state actually changed (useful for repaint gating).
    /// Enforces `max_len` if configured.
    pub fn handle_event(&mut self, event: &Event) -> bool {
        if let Some(cap) = self.max_len
            && self.input.value().chars().count() >= cap
        {
            // At the cap — allow deletion + navigation, but reject
            // character insertion. Inspect the key first; anything
            // that isn't a printable character insert falls through.
            if let Event::Key(key) = event
                && let ratatui::crossterm::event::KeyCode::Char(_) = key.code
            {
                return false;
            }
        }
        self.input.handle_event(event).is_some()
    }

    /// Render the input into `area`. When `focused`, positions the
    /// terminal cursor over the current insertion point.
    pub fn render(&self, area: Rect, frame: &mut Frame, block: Option<Block<'_>>, focused: bool) {
        let show_hint = self.input.value().is_empty() && !self.hint.is_empty();
        let text = if show_hint {
            &self.hint
        } else {
            self.input.value()
        };
        let style = if show_hint {
            Style::default().add_modifier(ratatui::style::Modifier::DIM)
        } else {
            Style::default()
        };

        // Compute the horizontal scroll so the cursor stays visible.
        // Subtract 2 for the border chrome when a block is provided; 3
        // gives one column of right-side breathing room. Fall back to
        // `area.width` for no-block renders.
        let inner_width = if block.is_some() {
            area.width.saturating_sub(3).max(1)
        } else {
            area.width.saturating_sub(1).max(1)
        };
        let scroll = self.input.visual_scroll(inner_width as usize);

        let has_block = block.is_some();
        let mut para = Paragraph::new(text).style(style).scroll((0, scroll as u16));
        if let Some(b) = block {
            para = para.block(b);
        }
        frame.render_widget(para, area);

        if focused && !show_hint {
            // Cursor placement — one column inside the border when a
            // block is provided, otherwise at the widget origin.
            let inset = u16::from(has_block);
            let x = self.input.visual_cursor().saturating_sub(scroll);
            frame.set_cursor_position((area.x + inset + x as u16, area.y + inset));
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        Terminal,
        backend::TestBackend,
        crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    };

    use super::*;

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    fn backspace() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE))
    }

    #[test]
    fn new_input_is_empty() {
        let i = TextInput::new();
        assert_eq!(i.value(), "");
    }

    #[test]
    fn preload_via_with_value() {
        let i = TextInput::new().with_value("hello");
        assert_eq!(i.value(), "hello");
    }

    #[test]
    fn typing_appends_characters() {
        let mut i = TextInput::new();
        i.handle_event(&key('h'));
        i.handle_event(&key('i'));
        assert_eq!(i.value(), "hi");
    }

    #[test]
    fn backspace_deletes() {
        let mut i = TextInput::new().with_value("abc");
        i.handle_event(&backspace());
        assert_eq!(i.value(), "ab");
    }

    #[test]
    fn max_len_rejects_further_chars() {
        let mut i = TextInput::new().with_max_len(3);
        i.handle_event(&key('a'));
        i.handle_event(&key('b'));
        i.handle_event(&key('c'));
        i.handle_event(&key('d'));
        assert_eq!(i.value(), "abc");
    }

    #[test]
    fn max_len_still_allows_backspace() {
        let mut i = TextInput::new().with_max_len(3);
        i.handle_event(&key('a'));
        i.handle_event(&key('b'));
        i.handle_event(&key('c'));
        i.handle_event(&backspace());
        assert_eq!(i.value(), "ab");
    }

    #[test]
    fn take_returns_and_clears() {
        let mut i = TextInput::new().with_value("abc");
        assert_eq!(i.take(), "abc");
        assert_eq!(i.value(), "");
    }

    #[test]
    fn clear_empties_buffer() {
        let mut i = TextInput::new().with_value("abc");
        i.clear();
        assert_eq!(i.value(), "");
    }

    #[test]
    fn handle_event_reports_state_change() {
        let mut i = TextInput::new();
        assert!(i.handle_event(&key('x')));
        // A key that neither inserts nor navigates (e.g. F5) returns
        // Some(_) from tui-input in some cases but not others — we
        // don't over-assert here. The character-insert path is the
        // one screens care about.
    }

    #[test]
    fn render_hint_shown_when_empty() {
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        let input = TextInput::new().with_hint("type here");
        term.draw(|frame| {
            input.render(frame.area(), frame, None, false);
        })
        .unwrap();
        let buf = term.backend().buffer();
        let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(row.contains("type here"), "expected hint text: {row:?}");
    }

    #[test]
    fn render_shows_typed_value_over_hint() {
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        let mut input = TextInput::new().with_hint("hint");
        input.handle_event(&key('x'));
        term.draw(|frame| {
            input.render(frame.area(), frame, None, false);
        })
        .unwrap();
        let buf = term.backend().buffer();
        let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(row.starts_with('x'), "expected typed char: {row:?}");
        assert!(
            !row.contains("hint"),
            "hint must not render when value present: {row:?}"
        );
    }
}
