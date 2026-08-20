//! Widget color animation + transformation effects.
//!
//! Runs at the pipeline layer, AFTER a widget renders its spans. Reads the
//! widget's `spec.metadata` for opt-in effects:
//!
//! | metadata key      | value                              | effect                              |
//! |-------------------|------------------------------------|-------------------------------------|
//! | `animate`         | `rainbow` \| `pulse` \| `sweep`    | time-based color cycle              |
//! | `cycleSeconds`    | `"30"`                             | cycle length (default 60)           |
//! | `gradientStart`   | `"#rrggbb"`                        | start color for static gradient     |
//! | `gradientEnd`     | `"#rrggbb"`                        | end color for static gradient     |
//! | `thresholds`      | `"50:green,80:yellow,100:red"`     | value-driven fg (traffic-light)     |
//! | `pulseAbove`      | `"90"` \| `"90%"` \| `"0.9"`       | pulse only when rendered pct >= N   |
//!
//! Precedence: `thresholds` wins (color pick), then `pulseAbove` decides
//! whether to pulse the (already colored) spans, then `animate`, then
//! static gradient, then no change. `thresholds` + `pulseAbove` compose —
//! set both to get "yellow at 70%, pulsed-red at 90%".
//!
//! Because Claude Code refreshes the statusline at its `refreshInterval`
//! (typically 10s), "animation" is really "one frame per refresh". The
//! cycle position is a deterministic function of wall-clock time, so
//! two invocations in the same second produce identical output — no
//! flicker.

use std::collections::BTreeMap;

use crate::{color::Color, settings::WidgetSpec, span::StyledSpan};

/// Apply per-widget animation / gradient / threshold effects to a run of
/// spans returned by [`Widget::render`](crate::widget::Widget::render).
///
/// Returns the (possibly re-styled) spans. When `spec.metadata` doesn't
/// declare any effect, this is a no-op and returns the input unchanged.
#[must_use]
pub fn apply(spans: Vec<StyledSpan>, spec: &WidgetSpec, now_ms: u64) -> Vec<StyledSpan> {
    let Some(meta) = spec.metadata.as_ref() else {
        return spans;
    };
    if meta.is_empty() {
        return spans;
    }

    // Shared cycle math used by pulse and pulseAbove.
    let cycle_s = meta
        .get("cycleSeconds")
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|s| *s > 0)
        .unwrap_or(60);
    let phase = if cycle_s == 0 {
        0.0
    } else {
        ((now_ms / 1000) % cycle_s) as f64 / cycle_s as f64
    };
    let sinusoidal_brightness = || 0.35 + 0.65 * (std::f64::consts::PI * phase).sin();

    // 1. Value-driven thresholds (traffic-light). Extracted from the
    //    rendered text via `\d+(\.\d+)?%` so no Widget-trait change
    //    is required. May be followed by pulseAbove; do NOT return
    //    early — fall through so the pulseAbove branch can animate
    //    the freshly-picked color.
    let mut spans = if let Some(threshold_str) = meta.get("thresholds")
        && let Some(value) = extract_percent(&spans)
        && let Some(color) = pick_threshold_color(threshold_str, value, now_ms)
    {
        apply_flat_fg(spans, color)
    } else {
        spans
    };

    // 2. `pulseAbove: "<N>"` — implicit pulse when the rendered percent
    //    crosses N. Composes with `thresholds` (color already picked
    //    above). Uses the same cycleSeconds/phase math as the explicit
    //    `animate: pulse` branch below.
    if let Some(threshold_str) = meta.get("pulseAbove")
        && let Some(threshold) = parse_percent_threshold(threshold_str)
        && let Some(value) = extract_percent(&spans)
        && value >= threshold
    {
        spans = apply_pulse(spans, sinusoidal_brightness());
        // pulseAbove is a conditional effect — once it fires (or doesn't),
        // fall through so `animate:` can still stack sweep or rainbow if
        // the user configured both. Keeping compose-forward semantics.
    }

    // 3. Time-based animations (explicit `animate:` key).
    match meta.get("animate").map(String::as_str) {
        Some("rainbow") => return apply_flat_rgb(spans, hsl_to_rgb(phase, 1.0, 0.5)),
        Some("pulse") => {
            return apply_pulse(spans, sinusoidal_brightness());
        }
        Some("sweep") => {
            if let Some((start, end)) = read_gradient_stops(meta) {
                return apply_sweep_gradient(spans, start, end, phase);
            }
        }
        _ => {}
    }

    // 4. Static per-character gradient (no animation).
    if let Some((start, end)) = read_gradient_stops(meta) {
        return apply_char_gradient(spans, start, end);
    }

    spans
}

/// Parse a `pulseAbove` metadata value into a percent (0.0..=100.0).
///
/// Accepts three shapes for user convenience:
/// - `"90"` — treated as 90 percent.
/// - `"90%"` — same as above.
/// - `"0.9"` — treated as 90 percent (fractional form).
///
/// Returns `None` for garbage or negative values. Values greater than 100
/// clamp to 100 (a pulseAbove of "150" means "pulse only at literally
/// max" — same as 100, still useful for "never pulse" via 101 if a user
/// really wants that shape).
fn parse_percent_threshold(spec: &str) -> Option<f64> {
    let trimmed = spec.trim().trim_end_matches('%');
    let value: f64 = trimmed.parse().ok()?;
    if value.is_nan() || value < 0.0 {
        return None;
    }
    // Fractional form: 0.9 → 90.
    let pct = if value <= 1.0 { value * 100.0 } else { value };
    Some(pct.min(100.0))
}

// -------- helpers --------

fn apply_flat_fg(spans: Vec<StyledSpan>, fg: Color) -> Vec<StyledSpan> {
    spans
        .into_iter()
        .map(|s| StyledSpan {
            fg: fg.clone(),
            ..s
        })
        .collect()
}

fn apply_flat_rgb(spans: Vec<StyledSpan>, rgb: (u8, u8, u8)) -> Vec<StyledSpan> {
    apply_flat_fg(
        spans,
        Color::Rgb {
            r: rgb.0,
            g: rgb.1,
            b: rgb.2,
        },
    )
}

/// Pulse existing fg by `brightness` (0.0..=1.0).
///
/// Base-color resolution: `Color::Rgb` uses its channels directly;
/// `Color::Named` resolves through [`crate::color::named_to_rgb`] so a
/// widget with `Color::Named("blue")` pulses the same VS-Code-blue the
/// static render shows. `Color::Default` (no fg configured) and any
/// unrecognised named color fall back to white, matching the pre-T4
/// behaviour for those cases.
fn apply_pulse(spans: Vec<StyledSpan>, brightness: f64) -> Vec<StyledSpan> {
    let clamp = |v: f64| -> u8 { v.clamp(0.0, 255.0).round() as u8 };
    spans
        .into_iter()
        .map(|s| {
            let base = match &s.fg {
                Color::Rgb { r, g, b } => (*r, *g, *b),
                Color::Named(name) => {
                    crate::color::named_to_rgb(name).unwrap_or((255, 255, 255))
                }
                Color::Default | Color::Ansi256(_) => (255, 255, 255),
            };
            let scaled = (
                clamp(f64::from(base.0) * brightness),
                clamp(f64::from(base.1) * brightness),
                clamp(f64::from(base.2) * brightness),
            );
            StyledSpan {
                fg: Color::Rgb {
                    r: scaled.0,
                    g: scaled.1,
                    b: scaled.2,
                },
                ..s
            }
        })
        .collect()
}

/// Character-by-character linear gradient across the whole widget's text.
fn apply_char_gradient(
    spans: Vec<StyledSpan>,
    start: (u8, u8, u8),
    end: (u8, u8, u8),
) -> Vec<StyledSpan> {
    let total_chars: usize = spans.iter().map(|s| s.text.chars().count()).sum();
    if total_chars <= 1 {
        return apply_flat_rgb(spans, start);
    }
    let mut out: Vec<StyledSpan> = Vec::with_capacity(total_chars);
    let mut idx = 0;
    for span in spans {
        for c in span.text.chars() {
            let t = idx as f64 / (total_chars - 1) as f64;
            let rgb = lerp_rgb(start, end, t);
            out.push(StyledSpan {
                text: c.to_string(),
                fg: Color::Rgb {
                    r: rgb.0,
                    g: rgb.1,
                    b: rgb.2,
                },
                bg: span.bg.clone(),
                bold: span.bold,
                dim: span.dim,
                italic: span.italic,
                underline: span.underline,
                gradient_hint: true,
                metadata_percent: None,
                flex_hint: false,
            });
            idx += 1;
        }
    }
    out
}

/// Same as [`apply_char_gradient`] but the mapping is offset by `phase`
/// (0.0..=1.0), so successive refreshes shift the color across the text.
fn apply_sweep_gradient(
    spans: Vec<StyledSpan>,
    start: (u8, u8, u8),
    end: (u8, u8, u8),
    phase: f64,
) -> Vec<StyledSpan> {
    let total_chars: usize = spans.iter().map(|s| s.text.chars().count()).sum();
    if total_chars == 0 {
        return spans;
    }
    let mut out: Vec<StyledSpan> = Vec::with_capacity(total_chars);
    let mut idx = 0;
    for span in spans {
        for c in span.text.chars() {
            let raw = idx as f64 / total_chars as f64 + phase;
            // Triangle wave: 0 -> 1 -> 0 across `raw`'s fractional part.
            let frac = raw - raw.floor();
            let t = if frac < 0.5 {
                frac * 2.0
            } else {
                (1.0 - frac) * 2.0
            };
            let rgb = lerp_rgb(start, end, t);
            out.push(StyledSpan {
                text: c.to_string(),
                fg: Color::Rgb {
                    r: rgb.0,
                    g: rgb.1,
                    b: rgb.2,
                },
                bg: span.bg.clone(),
                bold: span.bold,
                dim: span.dim,
                italic: span.italic,
                underline: span.underline,
                gradient_hint: true,
                metadata_percent: None,
                flex_hint: false,
            });
            idx += 1;
        }
    }
    out
}

type Rgb = (u8, u8, u8);

fn read_gradient_stops(meta: &BTreeMap<String, String>) -> Option<(Rgb, Rgb)> {
    let start = parse_hex(meta.get("gradientStart")?)?;
    let end = parse_hex(meta.get("gradientEnd")?)?;
    Some((start, end))
}

/// Pick the fg color for a threshold entry.
///
/// Format: `"cap:color[,cap:color]..."` where cap is a number and color is
/// either:
///   - a named color (`green`, `yellow`, `brightRed`, ...)
///   - a hex code (`#rrggbb`)
///   - a `|`-separated cycle (`#8b0000|#ff0000`) that alternates one
///     color per second driven by `now_ms`. Useful for a "flashing"
///     effect at the top threshold band.
fn pick_threshold_color(spec: &str, value: f64, now_ms: u64) -> Option<Color> {
    for entry in spec.split(',') {
        let mut it = entry.trim().splitn(2, ':');
        let cap = it.next()?.trim().parse::<f64>().ok()?;
        let color_spec = it.next()?.trim();
        if value <= cap {
            let alternatives: Vec<&str> = color_spec.split('|').collect();
            if alternatives.is_empty() {
                return None;
            }
            let idx = ((now_ms / 1000) as usize) % alternatives.len();
            return parse_color_spec(alternatives[idx]);
        }
    }
    None
}

/// Parse a single color spec: `"green"` -> [`Color::Named`],
/// `"#rrggbb"` -> [`Color::Rgb`].
fn parse_color_spec(s: &str) -> Option<Color> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() != 6 {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(Color::Rgb { r, g, b });
    }
    Some(Color::Named(s.to_string()))
}

fn extract_percent(spans: &[StyledSpan]) -> Option<f64> {
    // Prefer an explicit percent hint attached by the widget itself —
    // widgets whose rendered text doesn't include a literal `%`
    // (`context-length`, `tokens-*`, `cache-*`) attach a zero-width
    // sentinel span carrying `metadata_percent` so thresholds + pulseAbove
    // can still fire. Any span in the run may carry the hint; take the
    // first one found.
    if let Some(pct) = spans.iter().find_map(|s| s.metadata_percent) {
        return Some(pct);
    }
    // Fall back to text scan: first `<digits>[.<digits>]%` pattern wins.
    let text: String = spans.iter().map(|s| s.text.as_str()).collect();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == b'%' {
            return text[start..i].parse().ok();
        }
    }
    None
}

fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

/// Linear interpolation between two RGB triples. `t` is clamped to
/// `0.0..=1.0`.
fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| -> u8 {
        let xf = x as f64;
        let yf = y as f64;
        (xf + (yf - xf) * t).round().clamp(0.0, 255.0) as u8
    };
    (lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
}

/// HSL (h in 0..=1) to RGB. Saturation + luminance in 0..=1.
#[must_use]
pub fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = ((h % 1.0) + 1.0) % 1.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue_to_rgb = |p: f64, q: f64, t: f64| -> f64 {
        let t = if t < 0.0 {
            t + 1.0
        } else if t > 1.0 {
            t - 1.0
        } else {
            t
        };
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::WidgetSpec;

    fn plain(text: &str) -> Vec<StyledSpan> {
        vec![StyledSpan {
            text: text.into(),
            ..Default::default()
        }]
    }

    fn spec_with_meta(pairs: &[(&str, &str)]) -> WidgetSpec {
        let mut s = WidgetSpec::new("1", "test");
        s.metadata = Some(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        s
    }

    #[test]
    fn no_metadata_is_passthrough() {
        let spans = plain("hello");
        let spec = WidgetSpec::new("1", "test");
        let out = apply(spans.clone(), &spec, 0);
        assert_eq!(out, spans);
    }

    #[test]
    fn rainbow_produces_rgb_span() {
        let spec = spec_with_meta(&[("animate", "rainbow"), ("cycleSeconds", "6")]);
        let out = apply(plain("hi"), &spec, 0);
        assert!(matches!(out[0].fg, Color::Rgb { .. }));
    }

    #[test]
    fn rainbow_cycles_through_time() {
        let spec = spec_with_meta(&[("animate", "rainbow"), ("cycleSeconds", "60")]);
        let a = apply(plain("x"), &spec, 0);
        let b = apply(plain("x"), &spec, 30_000);
        assert_ne!(a[0].fg, b[0].fg);
    }

    #[test]
    fn pulse_on_named_blue_scales_actual_blue() {
        // Widget default color promotion happens at the pipeline layer, so
        // by the time animate::apply runs, spans carry Named(...) rather
        // than Default. Named colors must resolve to their RGB before
        // brightness scaling — the pre-T4 behaviour scaled from white.
        let mut spans = plain("x");
        spans[0].fg = Color::Named("blue".into());
        // cycleSeconds=2, now_ms=1000 -> phase=0.5 -> sin(pi/2)=1 -> brightness=1.0.
        let spec = spec_with_meta(&[("animate", "pulse"), ("cycleSeconds", "2")]);
        let out = apply(spans, &spec, 1_000);
        // At brightness=1.0, expect the full-blue RGB (36, 114, 200) from
        // the VS Code integrated-terminal palette.
        match out[0].fg {
            Color::Rgb { r, g, b } => {
                assert!((i32::from(r) - 36).abs() < 5, "r={r}");
                assert!((i32::from(g) - 114).abs() < 5, "g={g}");
                assert!((i32::from(b) - 200).abs() < 5, "b={b}");
            }
            _ => panic!("expected Rgb after pulse, got {:?}", out[0].fg),
        }
    }

    #[test]
    fn pulse_on_default_falls_back_to_white() {
        // Color::Default means no fg configured — animate can't invent a
        // base color, so it pulses white (grey→white sinusoid).
        let spans = plain("x");
        // spans[0].fg is Color::Default by default.
        let spec = spec_with_meta(&[("animate", "pulse"), ("cycleSeconds", "2")]);
        let out = apply(spans, &spec, 1_000);
        match out[0].fg {
            Color::Rgb { r, g, b } => {
                assert!(r > 200 && g > 200 && b > 200, "expected white-ish, got ({r},{g},{b})");
            }
            _ => panic!("expected Rgb"),
        }
    }

    #[test]
    fn pulse_scales_existing_rgb() {
        let mut spans = plain("x");
        spans[0].fg = Color::Rgb {
            r: 100,
            g: 100,
            b: 100,
        };
        // Phase = 0.5 => sin(pi/2) = 1 => brightness = 1.0.
        let spec = spec_with_meta(&[("animate", "pulse"), ("cycleSeconds", "2")]);
        let out = apply(spans, &spec, 1_000);
        match out[0].fg {
            Color::Rgb { r, .. } => assert!((r as i32 - 100).abs() < 5),
            _ => panic!("expected Rgb"),
        }
    }

    #[test]
    fn static_gradient_splits_per_char() {
        let spec = spec_with_meta(&[("gradientStart", "#000000"), ("gradientEnd", "#ff0000")]);
        let out = apply(plain("ab"), &spec, 0);
        assert_eq!(out.len(), 2);
        assert!(matches!(out[0].fg, Color::Rgb { r: 0, .. }));
        assert!(matches!(out[1].fg, Color::Rgb { r: 255, .. }));
    }

    #[test]
    fn sweep_gradient_shifts_with_phase() {
        let spec = spec_with_meta(&[
            ("animate", "sweep"),
            ("cycleSeconds", "10"),
            ("gradientStart", "#00ff00"),
            ("gradientEnd", "#0000ff"),
        ]);
        let a = apply(plain("hello"), &spec, 0);
        let b = apply(plain("hello"), &spec, 5_000);
        assert_ne!(a[0].fg, b[0].fg);
    }

    #[test]
    fn threshold_picks_correct_color() {
        let spec = spec_with_meta(&[("thresholds", "50:green,80:yellow,100:red")]);
        let low = apply(plain("Session: 25%"), &spec, 0);
        assert!(matches!(low[0].fg, Color::Named(ref n) if n == "green"));
        let mid = apply(plain("Session: 65%"), &spec, 0);
        assert!(matches!(mid[0].fg, Color::Named(ref n) if n == "yellow"));
        let hi = apply(plain("Session: 95%"), &spec, 0);
        assert!(matches!(hi[0].fg, Color::Named(ref n) if n == "red"));
    }

    #[test]
    fn threshold_accepts_hex_colors() {
        let spec = spec_with_meta(&[("thresholds", "75:#ff8c00,100:#8b0000")]);
        let orange = apply(plain("Ctx: 60%"), &spec, 0);
        assert!(matches!(
            orange[0].fg,
            Color::Rgb {
                r: 0xff,
                g: 0x8c,
                b: 0x00
            }
        ));
    }

    #[test]
    fn threshold_flash_alternates_between_colors() {
        let spec = spec_with_meta(&[("thresholds", "100:#8b0000|#ff0000")]);
        // now_ms=0 -> second 0 (even) -> first color = #8b0000
        let a = apply(plain("Ctx: 95%"), &spec, 0);
        assert!(matches!(
            a[0].fg,
            Color::Rgb {
                r: 0x8b,
                g: 0x00,
                b: 0x00
            }
        ));
        // now_ms=1000 -> second 1 (odd) -> second color = #ff0000
        let b = apply(plain("Ctx: 95%"), &spec, 1_000);
        assert!(matches!(
            b[0].fg,
            Color::Rgb {
                r: 0xff,
                g: 0x00,
                b: 0x00
            }
        ));
    }

    #[test]
    fn pulse_above_fires_only_when_percent_reaches_threshold() {
        let spec = spec_with_meta(&[("pulseAbove", "80"), ("cycleSeconds", "2")]);
        // 70% -> below threshold -> passthrough.
        let low = apply(plain("Ctx: 70%"), &spec, 1_000);
        assert!(matches!(low[0].fg, Color::Default), "expected passthrough at 70%");
        // 85% -> above threshold -> pulsed. brightness at now_ms=1000
        // (phase 0.5) is ~1.0, so fg becomes bright-white RGB.
        let mut hi_spans = plain("Ctx: 85%");
        hi_spans[0].fg = Color::Named("blue".into());
        let hi = apply(hi_spans, &spec, 1_000);
        assert!(matches!(hi[0].fg, Color::Rgb { .. }), "expected Rgb after pulse");
    }

    #[test]
    fn pulse_above_composes_with_thresholds() {
        // At 85%, thresholds picks brightRed AND pulseAbove pulses that
        // color. Result: pulsed RGB derived from brightRed base (241,76,76).
        let spec = spec_with_meta(&[
            ("thresholds", "60:cyan,80:yellow,90:brightRed"),
            ("pulseAbove", "80"),
            ("cycleSeconds", "2"),
        ]);
        let out = apply(plain("Ctx: 85%"), &spec, 1_000);
        match out[0].fg {
            Color::Rgb { r, g, b } => {
                // brightness ~= 1.0 -> ~= brightRed (241, 76, 76).
                assert!((i32::from(r) - 241).abs() < 10, "r={r}");
                assert!((i32::from(g) - 76).abs() < 10, "g={g}");
                assert!((i32::from(b) - 76).abs() < 10, "b={b}");
            }
            _ => panic!("expected Rgb after threshold+pulse"),
        }
    }

    #[test]
    fn pulse_above_accepts_various_formats() {
        for spec_val in ["90", "90%", "0.9"] {
            let spec = spec_with_meta(&[("pulseAbove", spec_val), ("cycleSeconds", "2")]);
            let out = apply(plain("Ctx: 95%"), &spec, 1_000);
            assert!(
                matches!(out[0].fg, Color::Rgb { .. }),
                "expected pulse to fire at 95% for pulseAbove={spec_val}"
            );
        }
    }

    #[test]
    fn pulse_above_no_op_without_percent_source() {
        // Text carries no percent AND there's no metadata_percent hint on
        // any span (T6 lands the hint path). pulseAbove must skip cleanly.
        let spec = spec_with_meta(&[("pulseAbove", "50"), ("cycleSeconds", "2")]);
        let out = apply(plain("Ctx: 78k tokens"), &spec, 1_000);
        assert!(matches!(out[0].fg, Color::Default), "expected passthrough with no % source");
    }

    #[test]
    fn parse_percent_threshold_shapes() {
        assert_eq!(parse_percent_threshold("90"), Some(90.0));
        assert_eq!(parse_percent_threshold("90%"), Some(90.0));
        assert_eq!(parse_percent_threshold(" 85 "), Some(85.0));
        assert_eq!(parse_percent_threshold("0.9"), Some(90.0));
        assert_eq!(parse_percent_threshold("1.0"), Some(100.0));
        assert_eq!(parse_percent_threshold("101"), Some(100.0));
        assert_eq!(parse_percent_threshold("-5"), None);
        assert_eq!(parse_percent_threshold("garbage"), None);
    }

    #[test]
    fn threshold_needs_percent_in_text() {
        let spec = spec_with_meta(&[("thresholds", "50:green,100:red")]);
        let out = apply(plain("no percent here"), &spec, 0);
        // No `%` -> effect skipped -> passthrough.
        assert!(matches!(out[0].fg, Color::Default));
    }

    #[test]
    fn extract_percent_prefers_hint_over_text() {
        // Both a literal 50% AND a hint of 90.0 — hint wins so widgets
        // that carry a computed percent don't have to strip their own
        // rendered numbers.
        let spans = vec![
            StyledSpan {
                text: "50%".into(),
                ..Default::default()
            },
            StyledSpan {
                text: String::new(),
                metadata_percent: Some(90.0),
                ..Default::default()
            },
        ];
        assert_eq!(extract_percent(&spans), Some(90.0));
    }

    #[test]
    fn extract_percent_falls_to_text_when_no_hint() {
        let spans = plain("Ctx: 42.5%");
        assert_eq!(extract_percent(&spans), Some(42.5));
    }

    #[test]
    fn threshold_fires_via_percent_hint_alone() {
        // No % in text, but a hint span pushes the value across the 80%
        // threshold — the "up to 100%" band picks brightRed.
        let spec = spec_with_meta(&[("thresholds", "80:red,100:brightRed")]);
        let mut spans = plain("Ctx: 170k");
        spans.push(StyledSpan {
            text: String::new(),
            metadata_percent: Some(85.0),
            ..Default::default()
        });
        let out = apply(spans, &spec, 0);
        assert!(matches!(out[0].fg, Color::Named(ref n) if n == "brightRed"));
    }

    #[test]
    fn pulse_above_fires_via_percent_hint_alone() {
        let spec = spec_with_meta(&[("pulseAbove", "80"), ("cycleSeconds", "2")]);
        let mut spans = plain("Ctx: 170k");
        spans[0].fg = Color::Named("blue".into());
        spans.push(StyledSpan {
            text: String::new(),
            metadata_percent: Some(85.0),
            ..Default::default()
        });
        let out = apply(spans, &spec, 1_000);
        assert!(matches!(out[0].fg, Color::Rgb { .. }));
    }

    #[test]
    fn hsl_to_rgb_endpoints() {
        // red at hue 0
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), (255, 0, 0));
        // green at hue 1/3
        assert_eq!(hsl_to_rgb(1.0 / 3.0, 1.0, 0.5), (0, 255, 0));
        // blue at hue 2/3
        assert_eq!(hsl_to_rgb(2.0 / 3.0, 1.0, 0.5), (0, 0, 255));
    }

    #[test]
    fn parse_hex_forms() {
        assert_eq!(parse_hex("#0a80ff"), Some((0x0a, 0x80, 0xff)));
        assert_eq!(parse_hex("0a80ff"), Some((0x0a, 0x80, 0xff)));
        assert_eq!(parse_hex("#zz"), None);
    }
}
