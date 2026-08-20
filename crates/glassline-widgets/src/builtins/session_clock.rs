//! `session-clock` — elapsed session time. Prefers `StatusJson.cost.
//! total_duration_ms`; falls back to the transcript-derived duration
//! computed by `glassline-render::transcript`. Port of TS `SessionClock.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{DurationFormat, format_duration_ms, is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(SessionClock)
}

pub struct SessionClock;

impl Widget for SessionClock {
    fn id(&self) -> &'static str {
        "session-clock"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::SESSION_CLOCK
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("yellow")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        // First choice: cost.total_duration_ms is authoritative and always
        // current, since Claude Code updates it on every refresh.
        let duration = ctx
            .data
            .as_ref()
            .and_then(|d| d.cost.as_ref())
            .and_then(|c| c.total_duration_ms)
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map(|ms| format_duration_ms(ms as u64, session_clock_fmt()));

        // Fall back to the transcript-scan-derived duration (populated by
        // the render binary when SESSION_CLOCK is in the requirements set).
        let text = duration
            .or_else(|| ctx.session_duration.clone())
            .unwrap_or_else(|| "0m".to_string());

        let rendered = if is_raw(spec) {
            text
        } else {
            format!("Session: {text}")
        };
        styled(spec, rendered)
    }
}

/// Session-clock-specific formatter options: `<1m` for sub-minute (never `0m` —
/// reads as broken), no day split (sessions are typically <24h; if a user does
/// run past 24h they see `27hr` not `1d 3hr`, matching TS).
fn session_clock_fmt() -> DurationFormat {
    DurationFormat {
        compact: false,
        use_days: false,
        less_than_min: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{Cost, StatusJson};

    fn ctx_from_cost(dur_ms: Option<f64>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                cost: Some(Cost {
                    total_duration_ms: dur_ms,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn uses_cost_duration_when_present() {
        let spans = SessionClock.render(
            &WidgetSpec::new("1", "session-clock"),
            &ctx_from_cost(Some(3_720_000.0)), // 62 min = 1hr 2m
        );
        assert_eq!(spans[0].text, "Session: 1hr 2m");
    }

    #[test]
    fn raw_drops_prefix() {
        let mut spec = WidgetSpec::new("1", "session-clock");
        spec.raw_value = Some(true);
        let spans = SessionClock.render(&spec, &ctx_from_cost(Some(600_000.0))); // 10m
        assert_eq!(spans[0].text, "10m");
    }

    #[test]
    fn falls_back_to_transcript_duration() {
        let ctx = RenderContext {
            data: Some(StatusJson::default()),
            session_duration: Some("42m".into()),
            ..Default::default()
        };
        let spans = SessionClock.render(&WidgetSpec::new("1", "session-clock"), &ctx);
        assert_eq!(spans[0].text, "Session: 42m");
    }

    #[test]
    fn defaults_to_zero_min_when_nothing_available() {
        let spans = SessionClock.render(
            &WidgetSpec::new("1", "session-clock"),
            &RenderContext::default(),
        );
        assert_eq!(spans[0].text, "Session: 0m");
    }

    #[test]
    fn under_one_minute() {
        let spans = SessionClock.render(
            &WidgetSpec::new("1", "session-clock"),
            &ctx_from_cost(Some(20_000.0)),
        );
        assert_eq!(spans[0].text, "Session: <1m");
    }
}
