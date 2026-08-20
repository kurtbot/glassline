//! Rendering context passed to every [`crate::screen::Screen::render`] call.
//!
//! T1.2 lands the minimal struct so `Screen` compiles. T1.10 fleshes out
//! `draw_panel` / `draw_modal` / `draw_preview` helpers.

use ratatui::Frame;

use glassline_core::settings::Settings;

/// The per-frame drawing context. Holds a mutable reference to the
/// current [`ratatui::Frame`] plus a read-only borrow of the app's
/// scratch [`Settings`], so screens can render widgets that reflect
/// unsaved edits.
pub struct Ui<'a, 'b> {
    /// The frame currently being drawn. Screens call
    /// [`Frame::render_widget`] and friends through this.
    pub frame: &'a mut Frame<'b>,
    /// Read-only view of the app's scratch settings — the shape the
    /// preview should render.
    pub settings: &'a Settings,
}

impl<'a, 'b> Ui<'a, 'b> {
    #[must_use]
    pub fn new(frame: &'a mut Frame<'b>, settings: &'a Settings) -> Self {
        Self { frame, settings }
    }
}
