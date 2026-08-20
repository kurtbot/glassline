//! Anthropic OAuth usage widgets: `session-usage`, `weekly-usage`,
//! `weekly-sonnet-usage`, `weekly-opus-usage`, `weekly-reset-timer`.
//!
//! MVP scope: labeled percentage (`Session: 45%`) or, for the timer,
//! the compact time-until-reset (`Weekly reset: 6h 42m`). The full TS
//! feature set (progress bar / slider / inverted / cursor / timezone /
//! locale / weekday / hours-only) lands with T-1.7f — usable core first.

use glassline_core::{
    render_context::{RenderContext, RenderUsageData, UsageError},
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{DurationFormat, duration_until_iso_ms, format_duration_ms, is_raw, styled};

// --------- percentage widgets ---------

#[derive(Copy, Clone)]
enum PercentKind {
    Session,
    Weekly,
    WeeklySonnet,
    WeeklyOpus,
}

impl PercentKind {
    fn id(self) -> &'static str {
        match self {
            PercentKind::Session => "session-usage",
            PercentKind::Weekly => "weekly-usage",
            PercentKind::WeeklySonnet => "weekly-sonnet-usage",
            PercentKind::WeeklyOpus => "weekly-opus-usage",
        }
    }
    fn label(self) -> &'static str {
        match self {
            PercentKind::Session => "Session: ",
            PercentKind::Weekly => "Weekly: ",
            PercentKind::WeeklySonnet => "Weekly Sonnet: ",
            PercentKind::WeeklyOpus => "Weekly Opus: ",
        }
    }
    fn value(self, u: &RenderUsageData) -> Option<f64> {
        match self {
            PercentKind::Session => u.session_usage,
            PercentKind::Weekly => u.weekly_usage,
            PercentKind::WeeklySonnet => u.weekly_sonnet_usage,
            PercentKind::WeeklyOpus => u.weekly_opus_usage,
        }
    }
}

pub struct UsagePercent(PercentKind);

impl Widget for UsagePercent {
    fn id(&self) -> &'static str {
        self.0.id()
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::USAGE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlue")
    }
    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(usage) = ctx.usage_data.as_ref() else {
            return Vec::new();
        };
        if let Some(err) = usage.error {
            return styled(spec, error_text(self.0.label(), err, is_raw(spec)));
        }
        let Some(pct) = self.0.value(usage) else {
            return Vec::new();
        };
        let formatted = format!("{pct:.0}%");
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("{}{}", self.0.label(), formatted)
        };
        styled(spec, text)
    }
}

pub fn session_usage_factory() -> Box<dyn Widget> {
    Box::new(UsagePercent(PercentKind::Session))
}
pub fn weekly_usage_factory() -> Box<dyn Widget> {
    Box::new(UsagePercent(PercentKind::Weekly))
}
pub fn weekly_sonnet_usage_factory() -> Box<dyn Widget> {
    Box::new(UsagePercent(PercentKind::WeeklySonnet))
}
pub fn weekly_opus_usage_factory() -> Box<dyn Widget> {
    Box::new(UsagePercent(PercentKind::WeeklyOpus))
}

// --------- reset timer ---------

pub struct WeeklyResetTimer;

impl Widget for WeeklyResetTimer {
    fn id(&self) -> &'static str {
        "weekly-reset-timer"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::USAGE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlue")
    }
    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(usage) = ctx.usage_data.as_ref() else {
            return Vec::new();
        };
        if let Some(err) = usage.error {
            return styled(spec, error_text("Weekly reset: ", err, is_raw(spec)));
        }
        let Some(iso) = usage.weekly_reset_at.as_deref() else {
            return Vec::new();
        };
        let Some(duration_ms) = duration_until_iso_ms(iso) else {
            return Vec::new();
        };
        let formatted = format_duration_ms(duration_ms, DurationFormat::default());
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("Weekly reset: {formatted}")
        };
        styled(spec, text)
    }
}

pub fn weekly_reset_timer_factory() -> Box<dyn Widget> {
    Box::new(WeeklyResetTimer)
}

// --------- helpers ---------

fn error_text(label: &str, err: UsageError, raw: bool) -> String {
    let msg = match err {
        UsageError::NoCredentials => "[No credentials]",
        UsageError::Timeout => "[Timeout]",
        UsageError::RateLimited => "[Rate limited]",
        UsageError::ApiError => "[API Error]",
        UsageError::ParseError => "[Parse Error]",
    };
    if raw {
        msg.to_string()
    } else {
        format!("{label}{msg}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(usage: RenderUsageData) -> RenderContext {
        RenderContext {
            usage_data: Some(usage),
            ..RenderContext::default()
        }
    }

    #[test]
    fn session_usage_labels_percent() {
        let spans = session_usage_factory().render(
            &WidgetSpec::new("1", "session-usage"),
            &ctx(RenderUsageData {
                session_usage: Some(45.0),
                ..Default::default()
            }),
        );
        assert_eq!(spans[0].text, "Session: 45%");
    }

    #[test]
    fn weekly_usage_labels_percent() {
        let spans = weekly_usage_factory().render(
            &WidgetSpec::new("1", "weekly-usage"),
            &ctx(RenderUsageData {
                weekly_usage: Some(55.0),
                ..Default::default()
            }),
        );
        assert_eq!(spans[0].text, "Weekly: 55%");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "weekly-usage");
        spec.raw_value = Some(true);
        let spans = weekly_usage_factory().render(
            &spec,
            &ctx(RenderUsageData {
                weekly_usage: Some(42.0),
                ..Default::default()
            }),
        );
        assert_eq!(spans[0].text, "42%");
    }

    #[test]
    fn error_replaces_percent_with_bracket_msg() {
        let spans = session_usage_factory().render(
            &WidgetSpec::new("1", "session-usage"),
            &ctx(RenderUsageData {
                error: Some(UsageError::NoCredentials),
                ..Default::default()
            }),
        );
        assert_eq!(spans[0].text, "Session: [No credentials]");
    }

    #[test]
    fn empty_when_no_usage_data() {
        let spans = session_usage_factory().render(
            &WidgetSpec::new("1", "session-usage"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn duration_formats_compact_hours_and_minutes() {
        // 6h 42m
        let ms = (6 * 3_600 + 42 * 60) * 1000;
        assert_eq!(format_duration_ms(ms, DurationFormat::default()), "6hr 42m");
    }

    #[test]
    fn duration_uses_days_split() {
        // 27h → 1d 3hr
        let ms = (27 * 3_600) * 1000;
        assert_eq!(format_duration_ms(ms, DurationFormat::default()), "1d 3hr");
    }

    #[test]
    fn duration_hours_only_form() {
        // 27h without day split
        let ms = (27 * 3_600) * 1000;
        assert_eq!(
            format_duration_ms(
                ms,
                DurationFormat {
                    use_days: false,
                    ..DurationFormat::default()
                }
            ),
            "27hr"
        );
    }

    #[test]
    fn duration_zero_returns_0m() {
        assert_eq!(format_duration_ms(0, DurationFormat::default()), "0m");
    }

    #[test]
    fn duration_compact_uses_h() {
        assert_eq!(
            format_duration_ms(
                3_600_000,
                DurationFormat {
                    compact: true,
                    use_days: false,
                    ..DurationFormat::default()
                }
            ),
            "1h"
        );
    }
}
