//! Hand-rolled ANSI SGR builder (design §4.4).
//!
//! Emits chalk-compatible sequences for a [`StyledSpan`] stream. P1 scope:
//! named colors (16-color palette) + bold + dim + italic + underline + reset.
//! Ansi256 + Rgb + gradient sweep land alongside the widgets that need them
//! in P1 T-1.2.

use std::fmt::Write;

use glassline_core::{color::Color, span::StyledSpan};

/// Write the ANSI-formatted representation of `spans` into `out`.
///
/// Emits a single `\x1b[0m` reset between distinct styles and again at the
/// end of the run so a downstream renderer never inherits our style.
pub fn write_spans<W: Write>(spans: &[StyledSpan], out: &mut W) -> std::fmt::Result {
    if spans.is_empty() {
        return Ok(());
    }
    for span in spans {
        if span.text.is_empty() {
            continue;
        }
        let sgr = build_sgr(span);
        if !sgr.is_empty() {
            out.write_str(&sgr)?;
        }
        out.write_str(&span.text)?;
        if !sgr.is_empty() {
            out.write_str("\x1b[0m")?;
        }
    }
    Ok(())
}

/// Render `spans` into an owned `String`. Convenience for the render path.
#[must_use]
pub fn spans_to_string(spans: &[StyledSpan]) -> String {
    let mut out = String::new();
    // Writing into a String never fails.
    let _ = write_spans(spans, &mut out);
    out
}

/// Build the SGR sequence for a single span (without any reset — callers
/// emit the reset after the span's text).
fn build_sgr(span: &StyledSpan) -> String {
    let mut codes: Vec<String> = Vec::new();
    if span.bold {
        codes.push("1".into());
    }
    if span.dim {
        codes.push("2".into());
    }
    if span.italic {
        codes.push("3".into());
    }
    if span.underline {
        codes.push("4".into());
    }
    if let Some(code) = fg_code(&span.fg) {
        codes.push(code);
    }
    if let Some(code) = bg_code(&span.bg) {
        codes.push(code);
    }
    if codes.is_empty() {
        return String::new();
    }
    format!("\x1b[{}m", codes.join(";"))
}

fn fg_code(color: &Color) -> Option<String> {
    match color {
        Color::Default => None,
        Color::Named(name) => named_code(name).map(|c| c.to_string()),
        Color::Ansi256(idx) => Some(format!("38;5;{idx}")),
        Color::Rgb { r, g, b } => Some(format!("38;2;{r};{g};{b}")),
    }
}

fn bg_code(color: &Color) -> Option<String> {
    match color {
        Color::Default => None,
        Color::Named(name) => named_code(name).map(|c| (c + 10).to_string()),
        Color::Ansi256(idx) => Some(format!("48;5;{idx}")),
        Color::Rgb { r, g, b } => Some(format!("48;2;{r};{g};{b}")),
    }
}

/// Map a chalk color name → base SGR code (foreground). Add `+10` for
/// background variants (SGR pattern: fg 30..=37 / 90..=97 → bg 40..=47 /
/// 100..=107).
fn named_code(name: &str) -> Option<u8> {
    // Shared normalisation lives in glassline_core::color so the RGB lookup
    // used by animate.rs stays in sync with what the ANSI writer accepts.
    match glassline_core::color::normalise_name(name).as_str() {
        "black" => Some(30),
        "red" => Some(31),
        "green" => Some(32),
        "yellow" => Some(33),
        "blue" => Some(34),
        "magenta" => Some(35),
        "cyan" => Some(36),
        "white" => Some(37),
        "bright-black" | "gray" | "grey" => Some(90),
        "bright-red" => Some(91),
        "bright-green" => Some(92),
        "bright-yellow" => Some(93),
        "bright-blue" => Some(94),
        "bright-magenta" => Some(95),
        "bright-cyan" => Some(96),
        "bright-white" => Some(97),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_span_has_no_sgr() {
        let span = StyledSpan::plain("hi");
        let out = spans_to_string(&[span]);
        assert_eq!(out, "hi");
    }

    #[test]
    fn named_fg_emits_expected_code() {
        let span = StyledSpan::named_fg("hi", "green");
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[32mhi\x1b[0m");
    }

    #[test]
    fn bold_alone_still_wraps_in_reset() {
        let mut span = StyledSpan::plain("bold");
        span.bold = true;
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[1mbold\x1b[0m");
    }

    #[test]
    fn multi_style_codes_are_semicolon_joined() {
        let mut span = StyledSpan::plain("x");
        span.bold = true;
        span.underline = true;
        span.fg = Color::Named("cyan".into());
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[1;4;36mx\x1b[0m");
    }

    #[test]
    fn ansi256_fg_uses_38_5_form() {
        let mut span = StyledSpan::plain("x");
        span.fg = Color::Ansi256(214);
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[38;5;214mx\x1b[0m");
    }

    #[test]
    fn rgb_fg_uses_38_2_form() {
        let mut span = StyledSpan::plain("x");
        span.fg = Color::Rgb {
            r: 10,
            g: 20,
            b: 30,
        };
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[38;2;10;20;30mx\x1b[0m");
    }

    #[test]
    fn camelcase_bright_name_normalises() {
        let span = StyledSpan::named_fg("x", "brightGreen");
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[92mx\x1b[0m");
    }

    #[test]
    fn kebab_bright_name_also_works() {
        let span = StyledSpan::named_fg("x", "bright-green");
        let out = spans_to_string(&[span]);
        assert_eq!(out, "\x1b[92mx\x1b[0m");
    }

    #[test]
    fn empty_span_text_is_skipped() {
        let span = StyledSpan::named_fg("", "red");
        let out = spans_to_string(&[span]);
        assert_eq!(out, "");
    }

    #[test]
    fn multiple_spans_concatenate_with_own_resets() {
        let a = StyledSpan::named_fg("a", "red");
        let b = StyledSpan::plain("b");
        let c = StyledSpan::named_fg("c", "cyan");
        let out = spans_to_string(&[a, b, c]);
        assert_eq!(out, "\x1b[31ma\x1b[0mb\x1b[36mc\x1b[0m");
    }

    #[test]
    fn unknown_named_color_falls_back_to_default() {
        let span = StyledSpan::named_fg("x", "not-a-color");
        let out = spans_to_string(&[span]);
        assert_eq!(out, "x");
    }
}
