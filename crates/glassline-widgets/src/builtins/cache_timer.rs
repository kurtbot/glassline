//! `cache-timer` — countdown for Claude Code's ephemeral prompt-cache TTL.
//! Port of upstream `CacheTimer.ts`.
//!
//! Anthropic's ephemeral prompt cache defaults to a 5-minute TTL; Claude
//! Code also writes 1-hour breakpoints (`cache_control ttl: "1h"`) for
//! the stable prefix. The expiry itself is never surfaced in the
//! transcript (only token counts are), so this widget's countdown is a
//! best-effort inference from the newest turn's timestamp.
//!
//! Rendering states (glyph slot in parentheses):
//!   - HOT (`symbolHot`, default `🔥`): a turn is in flight
//!     (`ctx.cache_timer.working = true`).
//!   - FRESH (`symbolFresh`, default `🟢`): remaining > 50% of adjusted TTL.
//!   - DRAINING (`symbolDraining`, default `🟡`): remaining 20-50%.
//!   - URGENT (`symbolUrgent`, default `🔴`): remaining 0-20%.
//!   - COLD (`symbolCold`, default `❄️`): elapsed >= TTL - safety margin.
//!
//! Metadata knobs:
//!   - `ttlSeconds` — 300 default, 3600 for the 1-hour tier, any positive
//!     integer > 5 accepted.
//!   - `hideWhenEmpty` — `"true"` hides the widget when the transcript
//!     hasn't touched the cache yet.
//!   - `symbolHot` / `symbolFresh` / `symbolDraining` / `symbolUrgent` /
//!     `symbolCold` — override the emoji for that state.

use glassline_core::{
    render_context::{CacheTimerState, RenderContext},
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{is_raw, styled};

const DEFAULT_TTL_SECONDS: u64 = 300;
const SAFETY_MARGIN_SECONDS: u64 = 5;

const HOT_DEFAULT: &str = "\u{1f525}"; // 🔥
const FRESH_DEFAULT: &str = "\u{1f7e2}"; // 🟢
const DRAINING_DEFAULT: &str = "\u{1f7e1}"; // 🟡
const URGENT_DEFAULT: &str = "\u{1f534}"; // 🔴
const COLD_DEFAULT: &str = "\u{2744}\u{fe0f}"; // ❄️

pub fn factory() -> Box<dyn Widget> {
    Box::new(CacheTimer)
}

pub struct CacheTimer;

impl Widget for CacheTimer {
    fn id(&self) -> &'static str {
        "cache-timer"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT | WidgetRequirements::CACHE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightCyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let hide_when_empty = hide_when_empty(spec);
        let ttl = ttl_seconds(spec);

        // No transcript path resolved / scanner didn't run → treat as
        // "no state yet". `hideWhenEmpty` opts out of the "n/a" fallback.
        let state = match ctx.cache_timer {
            Some(s) => s,
            None => {
                if hide_when_empty {
                    return Vec::new();
                }
                return styled(spec, labelled(spec, "n/a".to_string()));
            }
        };

        let body = render_body(&state, ttl, ctx.now_ms, spec, hide_when_empty);
        if body.is_empty() {
            return Vec::new();
        }
        styled(spec, body)
    }
}

fn render_body(
    state: &CacheTimerState,
    ttl_seconds: u64,
    now_ms: u64,
    spec: &WidgetSpec,
    hide_when_empty: bool,
) -> String {
    if state.working {
        return labelled(spec, with_glyph(&hot_symbol(spec), "HOT"));
    }
    let Some(last_touch_ms) = state.last_touch_ms else {
        if hide_when_empty {
            return String::new();
        }
        return labelled(spec, with_glyph(&cold_symbol(spec), "COLD"));
    };

    // Adjusted TTL: safety margin drops the visible timer earlier than
    // the real cache expiry so users don't spam commands in the last
    // few seconds only to hit a fresh MISS.
    let adjusted = ttl_seconds.saturating_sub(SAFETY_MARGIN_SECONDS);
    let elapsed_ms = now_ms.saturating_sub(last_touch_ms);
    let elapsed_secs = elapsed_ms / 1_000;

    if elapsed_secs >= adjusted {
        if hide_when_empty {
            return String::new();
        }
        return labelled(spec, with_glyph(&cold_symbol(spec), "COLD"));
    }

    let remaining = adjusted - elapsed_secs;
    let ratio = if adjusted == 0 {
        0.0
    } else {
        remaining as f64 / adjusted as f64
    };
    let glyph = if ratio > 0.5 {
        fresh_symbol(spec)
    } else if ratio > 0.2 {
        draining_symbol(spec)
    } else {
        urgent_symbol(spec)
    };
    labelled(spec, with_glyph(&glyph, &format_countdown(remaining)))
}

fn format_countdown(remaining_secs: u64) -> String {
    let m = remaining_secs / 60;
    let s = remaining_secs % 60;
    format!("{m}:{s:02}")
}

/// Join a glyph to countdown text. A blanked-out glyph collapses the
/// leading space so raw / no-glyph mode reads cleanly.
fn with_glyph(symbol: &str, text: &str) -> String {
    if symbol.is_empty() {
        text.to_string()
    } else {
        format!("{symbol} {text}")
    }
}

fn labelled(spec: &WidgetSpec, value: String) -> String {
    if is_raw(spec) {
        value
    } else {
        format!("Cache: {value}")
    }
}

fn ttl_seconds(spec: &WidgetSpec) -> u64 {
    let Some(raw) = spec
        .metadata
        .as_ref()
        .and_then(|m| m.get("ttlSeconds"))
    else {
        return DEFAULT_TTL_SECONDS;
    };
    let parsed: u64 = raw.parse().unwrap_or(0);
    if parsed > SAFETY_MARGIN_SECONDS {
        parsed
    } else {
        DEFAULT_TTL_SECONDS
    }
}

fn hide_when_empty(spec: &WidgetSpec) -> bool {
    spec.metadata
        .as_ref()
        .and_then(|m| m.get("hideWhenEmpty"))
        .is_some_and(|v| v == "true")
}

fn symbol(spec: &WidgetSpec, key: &str, default: &str) -> String {
    spec.metadata
        .as_ref()
        .and_then(|m| m.get(key))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}
fn hot_symbol(spec: &WidgetSpec) -> String {
    symbol(spec, "symbolHot", HOT_DEFAULT)
}
fn fresh_symbol(spec: &WidgetSpec) -> String {
    symbol(spec, "symbolFresh", FRESH_DEFAULT)
}
fn draining_symbol(spec: &WidgetSpec) -> String {
    symbol(spec, "symbolDraining", DRAINING_DEFAULT)
}
fn urgent_symbol(spec: &WidgetSpec) -> String {
    symbol(spec, "symbolUrgent", URGENT_DEFAULT)
}
fn cold_symbol(spec: &WidgetSpec) -> String {
    symbol(spec, "symbolCold", COLD_DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn spec_with(pairs: &[(&str, &str)]) -> WidgetSpec {
        let mut s = WidgetSpec::new("1", "cache-timer");
        s.metadata = Some(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect::<BTreeMap<String, String>>(),
        );
        s
    }

    fn ctx_with_cache(state: Option<CacheTimerState>, now_ms: u64) -> RenderContext {
        RenderContext {
            cache_timer: state,
            now_ms,
            ..RenderContext::default()
        }
    }

    #[test]
    fn no_state_renders_na_by_default() {
        let ctx = ctx_with_cache(None, 0);
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(spans[0].text.contains("n/a"));
    }

    #[test]
    fn no_state_hides_when_flagged() {
        let ctx = ctx_with_cache(None, 0);
        let spans = CacheTimer.render(&spec_with(&[("hideWhenEmpty", "true")]), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn working_state_renders_hot() {
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: true,
                last_touch_ms: None,
            }),
            10_000,
        );
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(spans[0].text.contains("HOT"));
        assert!(spans[0].text.starts_with("Cache: "));
    }

    #[test]
    fn cold_state_when_elapsed_exceeds_ttl() {
        // TTL 300, safety 5, adjusted 295. now_ms=1_000_000, last_touch=0.
        // elapsed=1000s > 295 -> COLD.
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            1_000_000,
        );
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(spans[0].text.contains("COLD"));
    }

    #[test]
    fn fresh_state_when_lots_of_time_left() {
        // TTL 300, safety 5, adjusted 295. elapsed=10s -> remaining=285,
        // ratio=285/295>0.5 -> FRESH glyph.
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            10_000,
        );
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(spans[0].text.contains(FRESH_DEFAULT), "got {:?}", spans[0].text);
    }

    #[test]
    fn draining_state_around_midpoint() {
        // adjusted=295, remaining ~= 100 -> ratio ~=0.34 -> DRAINING.
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            195_000, // elapsed=195s
        );
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(
            spans[0].text.contains(DRAINING_DEFAULT),
            "got {:?}",
            spans[0].text
        );
    }

    #[test]
    fn urgent_state_near_expiry() {
        // adjusted=295, remaining=30 -> ratio=0.10 -> URGENT.
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            265_000,
        );
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(
            spans[0].text.contains(URGENT_DEFAULT),
            "got {:?}",
            spans[0].text
        );
    }

    #[test]
    fn ttl_metadata_override_1h() {
        // TTL 3600, safety 5, adjusted 3595. elapsed=1000 -> remaining ~=2595 -> ratio >0.5 -> FRESH.
        let spec = spec_with(&[("ttlSeconds", "3600")]);
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            1_000_000,
        );
        let spans = CacheTimer.render(&spec, &ctx);
        assert!(spans[0].text.contains(FRESH_DEFAULT));
    }

    #[test]
    fn ttl_below_safety_falls_to_default() {
        // Any ttlSeconds <= SAFETY_MARGIN_SECONDS should silently fall back
        // to the 5-minute default.
        assert_eq!(ttl_seconds(&spec_with(&[("ttlSeconds", "3")])), 300);
        assert_eq!(ttl_seconds(&spec_with(&[("ttlSeconds", "0")])), 300);
        assert_eq!(ttl_seconds(&spec_with(&[("ttlSeconds", "not-a-number")])), 300);
    }

    #[test]
    fn symbol_override_via_metadata() {
        let spec = spec_with(&[("symbolFresh", "GREEN")]);
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            10_000,
        );
        let spans = CacheTimer.render(&spec, &ctx);
        assert!(
            spans[0].text.contains("GREEN"),
            "expected custom symbol, got {:?}",
            spans[0].text
        );
        assert!(!spans[0].text.contains(FRESH_DEFAULT));
    }

    #[test]
    fn raw_mode_drops_cache_prefix() {
        let mut spec = WidgetSpec::new("1", "cache-timer");
        spec.raw_value = Some(true);
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: Some(0),
            }),
            10_000,
        );
        let spans = CacheTimer.render(&spec, &ctx);
        assert!(!spans[0].text.starts_with("Cache:"));
    }

    #[test]
    fn countdown_formatted_mm_ss() {
        assert_eq!(format_countdown(0), "0:00");
        assert_eq!(format_countdown(9), "0:09");
        assert_eq!(format_countdown(65), "1:05");
        assert_eq!(format_countdown(3600), "60:00");
    }

    #[test]
    fn no_last_touch_but_not_working_renders_cold() {
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: None,
            }),
            10_000,
        );
        let spans = CacheTimer.render(&WidgetSpec::new("1", "cache-timer"), &ctx);
        assert!(spans[0].text.contains("COLD"));
    }

    #[test]
    fn no_last_touch_and_hide_when_empty_hides() {
        let ctx = ctx_with_cache(
            Some(CacheTimerState {
                working: false,
                last_touch_ms: None,
            }),
            10_000,
        );
        let spans = CacheTimer.render(&spec_with(&[("hideWhenEmpty", "true")]), &ctx);
        assert!(spans.is_empty());
    }
}
