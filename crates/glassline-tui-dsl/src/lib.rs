//! Thin DSL over `ratatui` + `crossterm` + `tui-input` for the glassline
//! layout config editor.
//!
//! Screens depend on this crate, never on `ratatui::*` directly, so the
//! rendering backend stays swappable. See
//! [[layout_config_editor_design_v1.1]] §4.3.

pub mod screen;
pub mod ui;

pub use screen::{Action, Outcome, Screen};
pub use ui::Ui;
