//! Shared helpers used across widgets — token formatting, label wrapping,
//! context-window derivation.

/// Shared mutex guarding env-var mutation across tests in this crate.
///
/// Cargo runs unit tests in parallel by default. Any test that calls
/// `std::env::set_var` / `remove_var` races other tests that read the
/// same variable. Tests that touch env state MUST take this lock at the
/// top:
///
/// ```ignore
/// let _guard = crate::common::TEST_ENV_LOCK.lock().unwrap();
/// ```
///
/// See [[widget_parity_design_v1.1]] §4.11 F3 for the design context.
#[cfg(test)]
pub static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

use glassline_core::{
    color::Color,
    settings::{DimSetting, WidgetSpec},
    span::StyledSpan,
    status_json::{ContextWindow, CurrentUsage, StatusJson},
};

/// Whether the widget's `rawValue` bit is set — TS ccstatusline uses this to
/// decide between `Ctx: 18.6k` (labelled) and `18.6k` (raw).
#[must_use]
pub fn is_raw(spec: &WidgetSpec) -> bool {
    spec.raw_value.unwrap_or(false)
}

/// Format `count` with `decimals` places in the k-range, promoting to `M`
/// once the k-value would round up to 1000 (e.g. 999_950 -> `1.0M` at 1 decimal
/// place). Port of `utils/format-tokens.ts`.
#[must_use]
pub fn format_tokens(count: u64, decimals: u32) -> String {
    let count_f = count as f64;
    let decimals_pow = 10f64.powi(decimals as i32);
    let m_threshold = 1_000_000.0 - 500.0 / decimals_pow;
    if count_f >= m_threshold {
        return format!("{:.1}M", count_f / 1_000_000.0);
    }
    if count >= 1_000 {
        let value = count_f / 1_000.0;
        return format!("{value:.dec$}k", dec = decimals as usize, value = value,);
    }
    count.to_string()
}

/// Wrap `value` in `label` unless the spec asks for the raw form.
/// Port of `formatRawOrLabeledValue` (`widgets/shared/raw-or-labeled.ts`).
#[must_use]
pub fn labeled_or_raw(spec: &WidgetSpec, label: &str, value: &str) -> String {
    if is_raw(spec) {
        value.to_string()
    } else {
        format!("{label}{value}")
    }
}

/// Build a single styled span honouring `spec.color`, `spec.background_color`,
/// `spec.bold`, and the ANSI-dim variant of `spec.dim` (`DimSetting::Bool(true)`).
///
/// `DimSetting::Parens` is a text-wrapping mode, not an ANSI attribute — it
/// belongs at the pipeline/animate layer and is intentionally NOT applied
/// here.
#[must_use]
pub fn styled(spec: &WidgetSpec, text: String) -> Vec<StyledSpan> {
    if text.is_empty() {
        return Vec::new();
    }
    let fg = spec
        .color
        .as_deref()
        .map_or(Color::Default, |c| Color::Named(c.to_string()));
    let bg = spec
        .background_color
        .as_deref()
        .map_or(Color::Default, |c| Color::Named(c.to_string()));
    let dim = matches!(spec.dim, Some(DimSetting::Bool(true)));
    vec![StyledSpan {
        text,
        fg,
        bg,
        bold: spec.bold.unwrap_or(false),
        dim,
        ..StyledSpan::default()
    }]
}

/// Derived stats over `StatusJson.context_window`, mirroring the shape of
/// TS `getContextWindowMetrics` at
/// `src/utils/context-window.ts:50`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ContextWindowMetrics {
    pub window_size: Option<u64>,
    pub used_tokens: Option<u64>,
    pub context_length_tokens: Option<u64>,
    pub used_percentage: Option<f64>,
    pub remaining_percentage: Option<f64>,
    pub total_input_tokens: Option<u64>,
    pub total_output_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
}

#[must_use]
pub fn context_window_metrics(data: Option<&StatusJson>) -> ContextWindowMetrics {
    let Some(cw) = data.and_then(|d| d.context_window.as_ref()) else {
        return ContextWindowMetrics::default();
    };
    context_window_metrics_from(cw)
}

fn context_window_metrics_from(cw: &ContextWindow) -> ContextWindowMetrics {
    let raw_window_size = finite_non_negative(cw.context_window_size);
    let window_size = raw_window_size.filter(|&v| v > 0.0).map(|v| v as u64);
    let total_input_tokens = finite_non_negative(cw.total_input_tokens).map(|v| v as u64);
    let total_output_tokens = finite_non_negative(cw.total_output_tokens).map(|v| v as u64);

    let mut current_usage_total: Option<u64> = None;
    let mut context_length_tokens: Option<u64> = None;
    let mut cached_tokens: Option<u64> = None;

    match cw.current_usage.as_ref() {
        Some(CurrentUsage::Number(n)) => {
            if let Some(v) = finite_non_negative(Some(*n)) {
                current_usage_total = Some(v as u64);
                context_length_tokens = current_usage_total;
            }
        }
        Some(CurrentUsage::Breakdown {
            input_tokens,
            output_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
        }) => {
            let input = finite_non_negative(*input_tokens).unwrap_or(0.0) as u64;
            let output = finite_non_negative(*output_tokens).unwrap_or(0.0) as u64;
            let creation = finite_non_negative(*cache_creation_input_tokens).unwrap_or(0.0) as u64;
            let read = finite_non_negative(*cache_read_input_tokens).unwrap_or(0.0) as u64;
            current_usage_total = Some(input + output + creation + read);
            context_length_tokens = Some(input + creation + read);
            cached_tokens = Some(creation + read);
        }
        None => {}
    }

    let raw_used_percentage = finite_non_negative(cw.used_percentage);
    let raw_remaining_percentage = finite_non_negative(cw.remaining_percentage);
    let used_tokens_from_pct = match (raw_used_percentage, window_size) {
        (Some(pct), Some(w)) => Some((pct / 100.0 * w as f64) as u64),
        _ => None,
    };
    let used_tokens = current_usage_total.or(used_tokens_from_pct);

    let used_percentage = if let Some(pct) = raw_used_percentage {
        Some(pct.clamp(0.0, 100.0))
    } else if let (Some(u), Some(w)) = (used_tokens, window_size)
        && w > 0
    {
        Some((u as f64 / w as f64 * 100.0).clamp(0.0, 100.0))
    } else {
        None
    };

    let remaining_percentage = if let Some(rem) = raw_remaining_percentage {
        Some(rem.clamp(0.0, 100.0))
    } else {
        used_percentage.map(|u| 100.0 - u)
    };

    let total_tokens =
        current_usage_total.or_else(|| match (total_input_tokens, total_output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            _ => None,
        });

    ContextWindowMetrics {
        window_size,
        used_tokens,
        context_length_tokens: context_length_tokens.or(used_tokens),
        used_percentage,
        remaining_percentage,
        total_input_tokens,
        total_output_tokens,
        cached_tokens,
        total_tokens,
    }
}

fn finite_non_negative(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x >= 0.0)
}

/// Options for [`format_duration_ms`]. Different widgets want different
/// tradeoffs — session-clock shows `<1m` because "0m" reads as broken, while
/// usage-reset timers use `0m` for "resets now". See [[widget_parity_design_v1.1]]
/// §4.11 F2 for the consolidation rationale.
#[derive(Debug, Clone, Copy)]
pub struct DurationFormat {
    /// `true` → `1h 2m`, `false` → `1hr 2m` (matches TS default).
    pub compact: bool,
    /// `true` → split total hours ≥24 into days (`1d 3hr`).
    pub use_days: bool,
    /// Sub-minute behavior. `true` → `<1m` (session-clock), `false` → `0m` (usage timer).
    pub less_than_min: bool,
}

impl Default for DurationFormat {
    fn default() -> Self {
        Self {
            compact: false,
            use_days: true,
            less_than_min: false,
        }
    }
}

/// Format a duration in milliseconds. Port of the union of TS
/// `formatUsageDuration` (usage-reset timers) and the session-clock
/// formatter. See [`DurationFormat`] for tradeoffs.
#[must_use]
pub fn format_duration_ms(ms: u64, fmt: DurationFormat) -> String {
    if ms < 60_000 {
        return if fmt.less_than_min {
            "<1m".to_string()
        } else {
            "0m".to_string()
        };
    }
    let total_minutes = ms / 60_000;
    let total_hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    let (days, hours) = if fmt.use_days {
        (total_hours / 24, total_hours % 24)
    } else {
        (0, total_hours)
    };
    let h_label = if fmt.compact { "h" } else { "hr" };
    let sep = if fmt.compact { "" } else { " " };
    let mut parts: Vec<String> = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}{h_label}"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if parts.is_empty() {
        // Reachable only when ms >= 60_000 but somehow the split zeroed
        // everything out (e.g. use_days=false + <60min case where total_hours=0).
        // Fall back to the sub-minute display convention.
        return if fmt.less_than_min {
            "<1m".to_string()
        } else {
            "0m".to_string()
        };
    }
    parts.join(sep)
}

/// Milliseconds until an RFC3339 timestamp elapses; `None` on parse failure.
/// `Some(0)` for past-timestamps (the widget layer decides how to render
/// "already elapsed" — timer-style widgets show "0m", countdown widgets may
/// switch labels).
#[must_use]
pub fn duration_until_iso_ms(iso: &str) -> Option<u64> {
    let ts =
        time::OffsetDateTime::parse(iso, &time::format_description::well_known::Rfc3339).ok()?;
    let now = time::OffsetDateTime::now_utc();
    let d = ts - now;
    if d.is_negative() {
        return Some(0);
    }
    Some(d.whole_milliseconds().max(0) as u64)
}

/// Milliseconds since an RFC3339 timestamp elapsed. Mirror of
/// [`duration_until_iso_ms`] pointing in the opposite direction. `None`
/// on parse failure; `Some(0)` when the timestamp is in the future.
#[must_use]
pub fn duration_since_iso_ms(iso: &str) -> Option<u64> {
    let ts =
        time::OffsetDateTime::parse(iso, &time::format_description::well_known::Rfc3339).ok()?;
    let now = time::OffsetDateTime::now_utc();
    let d = now - ts;
    if d.is_negative() {
        return Some(0);
    }
    Some(d.whole_milliseconds().max(0) as u64)
}

/// Default context window size when we can't derive one from Claude Code —
/// matches TS `MODEL_CONTEXT_DEFAULT_TOKENS`.
///
/// Users can override via the `CCSTATUSLINE_CONTEXT_SIZE_FALLBACK` env var in
/// TS; we accept the same name plus a `GLASSLINE_CONTEXT_SIZE_FALLBACK` so
/// the port doesn't break existing setups.
#[must_use]
pub fn default_context_window_size() -> u64 {
    if let Some(raw) = std::env::var_os("GLASSLINE_CONTEXT_SIZE_FALLBACK")
        .or_else(|| std::env::var_os("CCSTATUSLINE_CONTEXT_SIZE_FALLBACK"))
        && let Some(s) = raw.to_str()
        && let Ok(v) = s.parse::<u64>()
        && v > 0
    {
        return v;
    }
    200_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_ranges() {
        assert_eq!(format_tokens(0, 1), "0");
        assert_eq!(format_tokens(42, 1), "42");
        assert_eq!(format_tokens(999, 1), "999");
        assert_eq!(format_tokens(1_000, 1), "1.0k");
        assert_eq!(format_tokens(1_500, 1), "1.5k");
        assert_eq!(format_tokens(42_100, 1), "42.1k");
        assert_eq!(format_tokens(999_949, 1), "999.9k");
        assert_eq!(format_tokens(999_950, 1), "1.0M");
        assert_eq!(format_tokens(1_500_000, 1), "1.5M");
    }

    #[test]
    fn format_tokens_decimals_zero() {
        assert_eq!(format_tokens(1_500, 0), "2k");
        assert_eq!(format_tokens(999_499, 0), "999k");
        assert_eq!(format_tokens(999_500, 0), "1.0M");
    }

    #[test]
    fn styled_applies_dim_bool_true() {
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.dim = Some(DimSetting::Bool(true));
        let spans = styled(&spec, "hi".into());
        assert_eq!(spans.len(), 1);
        assert!(
            spans[0].dim,
            "DimSetting::Bool(true) must set StyledSpan.dim"
        );
    }

    #[test]
    fn styled_ignores_dim_parens() {
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.dim = Some(DimSetting::Parens(
            glassline_core::settings::ParensLiteral::Parens,
        ));
        let spans = styled(&spec, "hi".into());
        // Parens is a text-wrapping mode, not an ANSI attribute; styled()
        // must NOT set the ANSI dim bit for it.
        assert!(!spans[0].dim);
    }

    #[test]
    fn styled_dim_false_stays_off() {
        let mut spec = WidgetSpec::new("1", "custom-text");
        spec.dim = Some(DimSetting::Bool(false));
        let spans = styled(&spec, "hi".into());
        assert!(!spans[0].dim);
    }

    #[test]
    fn styled_dim_absent_stays_off() {
        let spec = WidgetSpec::new("1", "custom-text");
        let spans = styled(&spec, "hi".into());
        assert!(!spans[0].dim);
    }

    #[test]
    fn labeled_or_raw_switches_on_raw_value() {
        let mut spec = WidgetSpec::new("1", "ctx");
        assert_eq!(labeled_or_raw(&spec, "Ctx: ", "42.1k"), "Ctx: 42.1k");
        spec.raw_value = Some(true);
        assert_eq!(labeled_or_raw(&spec, "Ctx: ", "42.1k"), "42.1k");
    }

    #[test]
    fn context_metrics_from_breakdown() {
        let data = StatusJson {
            context_window: Some(ContextWindow {
                context_window_size: Some(200_000.0),
                current_usage: Some(CurrentUsage::Breakdown {
                    input_tokens: Some(80_000.0),
                    output_tokens: Some(5_000.0),
                    cache_creation_input_tokens: Some(10_000.0),
                    cache_read_input_tokens: Some(40_000.0),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let m = context_window_metrics(Some(&data));
        assert_eq!(m.window_size, Some(200_000));
        assert_eq!(m.context_length_tokens, Some(130_000));
        assert_eq!(m.cached_tokens, Some(50_000));
        assert!(matches!(m.used_percentage, Some(pct) if (pct - 67.5).abs() < 0.01));
    }

    #[test]
    fn context_metrics_from_percentage_only() {
        let data = StatusJson {
            context_window: Some(ContextWindow {
                context_window_size: Some(100_000.0),
                used_percentage: Some(42.5),
                ..Default::default()
            }),
            ..Default::default()
        };
        let m = context_window_metrics(Some(&data));
        assert_eq!(m.used_percentage, Some(42.5));
        assert_eq!(m.used_tokens, Some(42_500));
    }

    #[test]
    fn context_metrics_when_field_missing() {
        let m = context_window_metrics(Some(&StatusJson::default()));
        assert_eq!(m, ContextWindowMetrics::default());
    }

    #[test]
    fn default_window_size_env_override() {
        let _guard = TEST_ENV_LOCK.lock().unwrap();
        // Clean state, then set + verify.
        unsafe {
            std::env::remove_var("GLASSLINE_CONTEXT_SIZE_FALLBACK");
            std::env::set_var("CCSTATUSLINE_CONTEXT_SIZE_FALLBACK", "500000");
        }
        assert_eq!(default_context_window_size(), 500_000);
        unsafe {
            std::env::remove_var("CCSTATUSLINE_CONTEXT_SIZE_FALLBACK");
        }
        assert_eq!(default_context_window_size(), 200_000);
    }
}
