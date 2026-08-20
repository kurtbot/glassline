//! Intermediate render representation.
//!
//! [`StyledSpan`] is what a widget returns from `render`; the ANSI writer
//! walks a stream of spans and emits chalk-compatible SGR sequences.

use crate::color::Color;

/// A run of characters with a single style.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StyledSpan {
    pub text: String,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    /// Marks this span as part of a gradient sweep; the writer uses the
    /// gradient stored on the parent widget spec to color characters.
    pub gradient_hint: bool,
}

impl StyledSpan {
    /// A convenience constructor for a plain, unstyled string.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Self::default()
        }
    }

    /// Convenience: same as [`Self::plain`] with a named fg color.
    #[must_use]
    pub fn named_fg(text: impl Into<String>, fg_name: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            fg: Color::Named(fg_name.into()),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_span_has_no_style() {
        let span = StyledSpan::plain("hello");
        assert_eq!(span.text, "hello");
        assert!(matches!(span.fg, Color::Default));
        assert!(!span.bold);
    }

    #[test]
    fn named_fg_carries_name() {
        let span = StyledSpan::named_fg("main", "green");
        assert!(matches!(span.fg, Color::Named(ref n) if n == "green"));
    }
}
