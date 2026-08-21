//! Bottom-anchored floating notification. Non-blocking — the current
//! screen keeps focus.
//!
//! Screens schedule a toast by returning
//! [`crate::screen::Action::Toast(String)`] from `on_event`. The
//! [`crate::app::DslApp`] stores at most one active toast at a time; a
//! new toast replaces the previous. Toasts expire after their
//! configured duration; the app drops them on the next draw once
//! [`Toast::is_expired`] returns true.

use std::time::{Duration, Instant};

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

const DEFAULT_DURATION_MS: u64 = 2_500;

/// A single toast notification. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Toast {
    text: String,
    duration: Duration,
    created: Instant,
}

impl Toast {
    /// New toast with the default 2.5-second duration.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self::with_duration(text, Duration::from_millis(DEFAULT_DURATION_MS))
    }

    #[must_use]
    pub fn with_duration(text: impl Into<String>, duration: Duration) -> Self {
        Self {
            text: text.into(),
            duration,
            created: Instant::now(),
        }
    }

    /// The toast text — exposed so [`crate::app::DslApp::take_toast`]
    /// can pull it out for legacy string-only assertions in tests.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// `true` once the toast has outlived its configured duration.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.created.elapsed() >= self.duration
    }

    /// Render into the bottom row of `outer_area`. Emits nothing when
    /// expired — callers can call `is_expired` first to skip the draw,
    /// but this method still degrades safely on stale toasts.
    pub fn render(&self, outer_area: Rect, frame: &mut Frame) {
        if self.is_expired() || outer_area.height == 0 {
            return;
        }
        let row = Rect {
            x: outer_area.x,
            y: outer_area.y + outer_area.height.saturating_sub(1),
            width: outer_area.width,
            height: 1,
        };
        let block = Block::default().borders(Borders::NONE);
        let para = Paragraph::new(self.text.as_str())
            .block(block)
            .alignment(Alignment::Right)
            .style(Style::default().add_modifier(Modifier::REVERSED));
        frame.render_widget(para, row);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn new_default_duration_is_alive_immediately() {
        let t = Toast::new("hi");
        assert!(!t.is_expired());
    }

    #[test]
    fn very_short_duration_expires_after_sleep() {
        let t = Toast::with_duration("hi", Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.is_expired());
    }

    #[test]
    fn text_accessor_reads_back() {
        assert_eq!(Toast::new("saved").text(), "saved");
    }

    #[test]
    fn renders_at_bottom_row_when_alive() {
        let backend = TestBackend::new(20, 4);
        let mut term = Terminal::new(backend).unwrap();
        let toast = Toast::new("Saved!");
        term.draw(|frame| toast.render(frame.area(), frame))
            .unwrap();
        let buf = term.backend().buffer();
        let bottom_row: String = (0..20).map(|x| buf[(x, 3)].symbol().to_string()).collect();
        assert!(
            bottom_row.contains("Saved!"),
            "expected toast on bottom row: {bottom_row:?}"
        );
        let top_row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(
            !top_row.contains("Saved!"),
            "toast must not leak onto top row: {top_row:?}"
        );
    }

    #[test]
    fn expired_toast_does_not_render() {
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        let t = Toast::with_duration("stale", Duration::from_nanos(1));
        std::thread::sleep(Duration::from_millis(5));
        term.draw(|frame| t.render(frame.area(), frame)).unwrap();
        let buf = term.backend().buffer();
        for y in 0..3 {
            let row: String = (0..20).map(|x| buf[(x, y)].symbol().to_string()).collect();
            assert!(
                !row.contains("stale"),
                "expired toast leaked onto row {y}: {row:?}"
            );
        }
    }
}
