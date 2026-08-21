//! Thin DSL over `ratatui` + `crossterm` + `tui-input` for the glassline
//! layout config editor.
//!
//! Screens depend on this crate, never on `ratatui::*` directly, so the
//! rendering backend stays swappable without touching downstream code.

pub mod app;
pub mod list;
pub mod modal;
pub mod panel;
pub mod preview;
pub mod screen;
pub mod text_input;
pub mod toast;
pub mod ui;

pub use app::{DslApp, DslError};
pub use list::List;
pub use modal::{Button, Modal, centered_rect};
pub use panel::{BorderStyle, Panel};
pub use preview::{Preview, strip_ansi};
pub use screen::{Action, Outcome, Screen};
pub use text_input::TextInput;
pub use toast::Toast;
pub use ui::Ui;
