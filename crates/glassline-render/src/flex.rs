//! Flex-align pass — expand `flex_hint` sentinel spans to fill remaining
//! terminal width. Port of upstream's `FLEX_SENTINEL` handling in
//! `sirmalloc/ccstatusline/src/utils/renderer.ts`.
//!
//! Called by the pipeline once per rendered line, after all widget
//! `render()` calls but before ANSI serialization. Skipped when powerline
//! mode is enabled (powerline has its own alignment math) or when the
//! terminal width is unknown (no reliable target to fill).
//!
//! Algorithm:
//!   1. Find every span with `flex_hint = true`.
//!   2. If none, or `terminal_width = None`, or powerline mode → no-op.
//!   3. Sum the character widths of all non-flex spans (`content_width`).
//!   4. `remaining = max(0, terminal_width - content_width)`.
//!   5. `per_slot = remaining / flex_count`; `remainder = remaining % flex_count`.
//!   6. Rewrite each flex span's `text` to `" ".repeat(per_slot)`, with
//!      the last flex slot absorbing the modulo remainder.
//!
//! Character-width uses `chars().count()` — same simplification the ANSI
//! writer uses. Full grapheme-cluster / wide-char handling is a v1.1
//! concern; upstream ccstatusline is single-codepoint-per-cell as well.

use glassline_core::span::StyledSpan;

/// Expand any `flex_hint` sentinels in-place. See module docs for the
/// algorithm.
pub fn apply(spans: &mut [StyledSpan], terminal_width: Option<usize>, powerline: bool) {
    if powerline {
        return;
    }
    let Some(width) = terminal_width else {
        return;
    };
    let flex_positions: Vec<usize> = spans
        .iter()
        .enumerate()
        .filter(|(_, s)| s.flex_hint)
        .map(|(i, _)| i)
        .collect();
    if flex_positions.is_empty() {
        return;
    }
    let content_width: usize = spans.iter().map(|s| s.text.chars().count()).sum();
    let remaining = width.saturating_sub(content_width);
    if remaining == 0 {
        return;
    }
    let n = flex_positions.len();
    let per_slot = remaining / n;
    let remainder = remaining % n;
    for (nth, idx) in flex_positions.iter().enumerate() {
        let extra = if nth + 1 == n { remainder } else { 0 };
        spans[*idx].text = " ".repeat(per_slot + extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(text: &str) -> StyledSpan {
        StyledSpan {
            text: text.into(),
            ..Default::default()
        }
    }

    fn flex() -> StyledSpan {
        StyledSpan {
            text: String::new(),
            flex_hint: true,
            ..Default::default()
        }
    }

    #[test]
    fn no_flex_no_change() {
        let mut spans = vec![plain("hello"), plain(" world")];
        apply(&mut spans, Some(20), false);
        assert_eq!(spans[0].text, "hello");
        assert_eq!(spans[1].text, " world");
    }

    #[test]
    fn single_flex_absorbs_remainder() {
        // Content: "abc" (3) + flex + "de" (2) = 5. Width 20 -> 15 spaces.
        let mut spans = vec![plain("abc"), flex(), plain("de")];
        apply(&mut spans, Some(20), false);
        assert_eq!(spans[1].text, " ".repeat(15));
        assert_eq!(spans[0].text, "abc");
        assert_eq!(spans[2].text, "de");
    }

    #[test]
    fn two_flex_split_evenly_no_remainder() {
        // Content 4, two flex, width 20 -> 16 remaining -> 8 + 8.
        let mut spans = vec![plain("A"), flex(), plain("BB"), flex(), plain("C")];
        apply(&mut spans, Some(20), false);
        assert_eq!(spans[1].text, " ".repeat(8));
        assert_eq!(spans[3].text, " ".repeat(8));
    }

    #[test]
    fn remainder_lands_on_last_slot() {
        // Content 3, three flex, width 21 -> 18 remaining, 6/6/6.
        let mut spans = vec![plain("abc"), flex(), flex(), flex()];
        apply(&mut spans, Some(21), false);
        assert_eq!(spans[1].text.len(), 6);
        assert_eq!(spans[2].text.len(), 6);
        assert_eq!(spans[3].text.len(), 6);

        // Content 4, three flex, width 20 -> 16 remaining, 5/5/6 (remainder to last).
        let mut spans = vec![plain("abcd"), flex(), flex(), flex()];
        apply(&mut spans, Some(20), false);
        assert_eq!(spans[1].text.len(), 5);
        assert_eq!(spans[2].text.len(), 5);
        assert_eq!(spans[3].text.len(), 6);
    }

    #[test]
    fn no_terminal_width_is_noop() {
        let mut spans = vec![plain("abc"), flex(), plain("de")];
        apply(&mut spans, None, false);
        assert!(spans[1].text.is_empty(), "flex sentinel must stay empty");
    }

    #[test]
    fn powerline_short_circuits() {
        let mut spans = vec![plain("abc"), flex(), plain("de")];
        apply(&mut spans, Some(80), true);
        assert!(
            spans[1].text.is_empty(),
            "powerline mode must not expand flex sentinels"
        );
    }

    #[test]
    fn zero_remaining_is_noop() {
        // Content already fills the whole width — no room to expand.
        let mut spans = vec![plain("hello"), flex(), plain("world")];
        apply(&mut spans, Some(10), false);
        assert!(spans[1].text.is_empty());
    }

    #[test]
    fn narrower_than_content_saturates_to_zero() {
        let mut spans = vec![plain("hello world"), flex()];
        apply(&mut spans, Some(5), false);
        assert!(spans[1].text.is_empty());
    }

    #[test]
    fn multibyte_content_measured_by_chars_not_bytes() {
        // Content is 2 chars but 6 bytes (each `を` is 3 bytes in UTF-8).
        let mut spans = vec![plain("をを"), flex()];
        apply(&mut spans, Some(10), false);
        // width 10 - 2 chars = 8 spaces expected.
        assert_eq!(spans[1].text.len(), 8);
    }
}
