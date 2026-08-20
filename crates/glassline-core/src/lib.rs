// SPDX-FileCopyrightText: 2026 Kurt Milan
// SPDX-License-Identifier: MIT

//! Core domain types for glassline.
//!
//! Split by concern:
//! - [`status_json`] — the JSON payload Claude Code writes to glassline's stdin.
//! - [`settings`] — user-editable configuration (`settings.json`).
//! - [`widget`] — [`Widget`](widget::Widget) trait every widget implements.
//! - [`span`] — [`StyledSpan`](span::StyledSpan) — the intermediate representation
//!   between a widget's `render` call and the ANSI writer.
//! - [`color`] — color model (`Named` / `Ansi256` / `Rgb`) + gradient stops.
//! - [`protocol`] — v1 wire types for external-binary widgets (design §4.11).
//! - [`migration`] — settings.json schema evolution (design §4.12).
//! - [`render_context`] — everything a widget needs to render.

pub mod animate;
pub mod color;
pub mod migration;
pub mod protocol;
pub mod render_context;
pub mod settings;
pub mod span;
pub mod status_json;
pub mod widget;

pub use color::{Color, ColorLevel};
pub use render_context::RenderContext;
pub use settings::Settings;
pub use span::StyledSpan;
pub use status_json::StatusJson;
pub use widget::{Widget, WidgetRequirements};
