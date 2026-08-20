//! Thin DSL over `ratatui` + `crossterm` + `tui-input` for the glassline
//! layout config editor.
//!
//! Screens depend on this crate, never on `ratatui::*` directly, so the
//! rendering backend stays swappable. See
//! [[layout_config_editor_design_v1.1]] §4.3.
//!
//! **Status:** P1 scaffold (T1.1) — module tree lands empty; primitives
//! wire up in T1.2-T1.12.
