//! Rendering context passed to every [`crate::screen::Screen::render`] call.
//!
//! Wraps the current `Frame` + the app's scratch [`Settings`] so screens
//! can render widgets that reflect unsaved edits. Also exposes a small
//! surface of `frame`-forwarding helpers so common draw calls read
//! naturally at the screen call-site.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    widgets::Widget,
};

use glassline_core::settings::Settings;

/// The per-frame drawing context. Holds a mutable reference to the
/// current [`ratatui::Frame`] plus a read-only borrow of the app's
/// scratch [`Settings`].
pub struct Ui<'a, 'b> {
    /// The frame currently being drawn. Screens can reach in when they
    /// need something the helper surface doesn't cover — e.g. modals
    /// that want to draw over the reserved footer row.
    pub frame: &'a mut Frame<'b>,
    /// Read-only view of the app's scratch settings — the shape the
    /// preview should render.
    pub settings: &'a Settings,
    /// Frame area minus any rows reserved by the app for global chrome
    /// (footer keybindings strip). Screens should lay out inside this
    /// via [`Self::area`]. Overlays that want to paint anywhere
    /// (modals) can go through [`Self::frame`] directly.
    body_area: Rect,
}

impl<'a, 'b> Ui<'a, 'b> {
    #[must_use]
    pub fn new(frame: &'a mut Frame<'b>, settings: &'a Settings) -> Self {
        let body_area = frame.area();
        Self {
            frame,
            settings,
            body_area,
        }
    }

    /// Construct a `Ui` whose body area excludes the last `footer_rows`
    /// rows of the frame. Called by [`crate::app::DslApp`] to reserve
    /// the global-footer strip. Screens don't need to know footer
    /// rows exist — [`Self::area`] already excludes them.
    #[must_use]
    pub fn with_reserved_footer(
        frame: &'a mut Frame<'b>,
        settings: &'a Settings,
        footer_rows: u16,
    ) -> Self {
        let full = frame.area();
        let body_area = Rect {
            x: full.x,
            y: full.y,
            width: full.width,
            height: full.height.saturating_sub(footer_rows),
        };
        Self {
            frame,
            settings,
            body_area,
        }
    }

    /// The body drawing area (frame area minus the app's reserved
    /// footer strip).
    #[must_use]
    pub fn area(&self) -> Rect {
        self.body_area
    }

    /// The full frame area, ignoring any reserved footer strip. Modals
    /// and overlays that intentionally paint over chrome use this.
    #[must_use]
    pub fn frame_area(&self) -> Rect {
        self.frame.area()
    }

    /// Passthrough to [`Frame::render_widget`].
    pub fn render_widget<W: Widget>(&mut self, widget: W, area: Rect) {
        self.frame.render_widget(widget, area);
    }

    /// Passthrough to [`Frame::set_cursor_position`].
    pub fn set_cursor_position(&mut self, position: impl Into<Position>) {
        self.frame.set_cursor_position(position);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend, widgets::Paragraph};

    use super::*;

    #[test]
    fn area_returns_frame_bounds() {
        let backend = TestBackend::new(20, 5);
        let mut term = Terminal::new(backend).unwrap();
        let settings = Settings::default();
        term.draw(|frame| {
            let ui = Ui::new(frame, &settings);
            let a = ui.area();
            assert_eq!(a.width, 20);
            assert_eq!(a.height, 5);
        })
        .unwrap();
    }

    #[test]
    fn render_widget_paints_into_frame() {
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        let settings = Settings::default();
        term.draw(|frame| {
            let mut ui = Ui::new(frame, &settings);
            let area = ui.area();
            ui.render_widget(Paragraph::new("hello"), area);
        })
        .unwrap();
        let buf = term.backend().buffer();
        let row: String = (0..20).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(
            row.starts_with("hello"),
            "expected 'hello' in buffer: {row:?}"
        );
    }
}
