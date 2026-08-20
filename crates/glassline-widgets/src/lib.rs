// SPDX-FileCopyrightText: 2026 Kurt Milan
// SPDX-License-Identifier: MIT

//! Built-in widget implementations, indexed by ID via [`registry`].
//!
//! P1 vertical slice ships just `custom-text`; the rest land through P1
//! T-1.8..T-1.22 and P3.

#![cfg_attr(test, allow(unsafe_code))]

pub mod builtins;
pub mod common;
pub mod git;
pub mod jj;
pub mod registry;

pub use registry::{WIDGETS, WidgetFactory, resolve};
