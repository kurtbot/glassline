// SPDX-FileCopyrightText: 2026 Kurt Milan
// SPDX-License-Identifier: MIT

//! Hot-path render pipeline as a library so both `main.rs` and integration
//! tests / the TUI live-preview screen can call the same code.
//!
//! P1 vertical slice: reads a [`StatusJson`], walks `settings.lines`,
//! dispatches to each widget's `render`, and emits ANSI to a writer.
//! Full renderer port (powerline, gradient, flex, separator advance,
//! max-width computation) lands over T-1.23–T-1.24.

pub mod ansi;
pub mod config;
pub mod demo;
pub mod import;
pub mod install;
pub mod pipeline;
pub mod render_cache;
pub mod stdin_reader;
pub mod transcript;
pub mod usage;

pub use pipeline::{PipelineError, render_to_string};
