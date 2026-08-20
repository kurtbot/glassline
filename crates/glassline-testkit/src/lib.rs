// SPDX-FileCopyrightText: 2026 Kurt Milan
// SPDX-License-Identifier: MIT

//! Glassline test harness: fixtures, ANSI normalisation, parity gate.
//!
//! Design §4.10 — the parity gate is **visual-equivalence**, not byte-exact.
//! [`normalise`] converts raw ANSI into a stable
//! [`AttrRuns`] form; [`assert_visually_equivalent`] compares two ANSI blobs
//! by their normalised form.

pub mod fixture;
pub mod normalise;
pub mod ts_shim;

pub use fixture::{Fixture, FixtureLoader};
pub use normalise::{AttrRun, AttrRuns, AttrSet, normalise};

/// Assert that two ANSI-bearing strings render visually equivalent.
///
/// Panics with a human-diffable YAML-style diff when they differ.
pub fn assert_visually_equivalent(actual: &str, expected: &str) {
    let a = normalise(actual);
    let b = normalise(expected);
    if a == b {
        return;
    }
    panic!(
        "visual equivalence failed\n--- actual ---\n{a}\n--- expected ---\n{b}\n",
        a = a.debug_yaml(),
        b = b.debug_yaml(),
    );
}
