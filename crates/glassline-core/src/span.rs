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
    /// Percent hint for animation effects.
    ///
    /// Widgets whose rendered text is NOT a percent (`context-length`
    /// shows `Ctx: 78.6k`, tokens/cache widgets show token counts) attach
    /// a zero-width sentinel span carrying `metadata_percent = Some(pct)`
    /// so `animate.rs`'s `thresholds` and `pulseAbove` effects can fire
    /// based on the widget's underlying percent even when the display
    /// doesn't contain a literal `%` character.
    ///
    /// The ANSI writer already skips empty-text spans, so the hint span
    /// never contributes to visible output.
    pub metadata_percent: Option<f64>,
    /// Marker for flex-align expansion.
    ///
    /// Emitted by the `flex-separator` widget (an empty-text sentinel) so
    /// the render pipeline's `flex::apply` pass can find slots to
    /// distribute remaining terminal width across. Ports upstream's
    /// `FLEX_SENTINEL` renderer indirection into a typed field rather than
    /// an in-band string sentinel.
    ///
    /// The ANSI writer skips empty-text spans, so a `flex_hint` sentinel
    /// with an empty `text` renders nothing; `flex::apply` rewrites `text`
    /// to N spaces before the writer sees the span.
    pub flex_hint: bool,
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
