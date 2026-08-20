//! `input-speed` / `output-speed` / `total-speed` — tokens per second
//! computed from the transcript-derived [`SpeedMetrics`]. Ports of TS
//! `InputSpeed.ts` / `OutputSpeed.ts` / `TotalSpeed.ts` (which share the
//! `renderSpeedWidgetValue` helper).
//!
//! When `metadata.speedWindow` is set (e.g. `"5m"`, `"1h"`), the widget
//! reads the matching entry from `ctx.windowed_speed_metrics`. If the
//! window key isn't present, falls back to `ctx.speed_metrics` (session
//! average). Windowed keys are populated by the render binary's
//! transcript scanner — this widget just reads them.

use glassline_core::{
    render_context::{RenderContext, SpeedMetrics},
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

#[derive(Copy, Clone)]
enum Kind {
    Input,
    Output,
    Total,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Input => "In: ",
            Kind::Output => "Out: ",
            Kind::Total => "Total: ",
        }
    }
    fn id(self) -> &'static str {
        match self {
            Kind::Input => "input-speed",
            Kind::Output => "output-speed",
            Kind::Total => "total-speed",
        }
    }
    fn compute(self, m: &SpeedMetrics) -> Option<f64> {
        match self {
            Kind::Input => m.input_per_sec(),
            Kind::Output => m.output_per_sec(),
            Kind::Total => m.total_per_sec(),
        }
    }
}

/// Wraps [`Kind`] behind the [`Widget`] trait.
pub struct SpeedWidget(Kind);

impl Widget for SpeedWidget {
    fn id(&self) -> &'static str {
        self.0.id()
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::SPEED
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }
    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let metrics = pick_metrics(spec, ctx);
        let Some(metrics) = metrics else {
            return Vec::new();
        };
        let value = self.0.compute(metrics);
        let formatted = format_speed(value);
        styled(spec, labeled_or_raw(spec, self.0.label(), &formatted))
    }
}

/// Resolve which `SpeedMetrics` snapshot the widget should read.
///
/// Preference order:
/// 1. `ctx.windowed_speed_metrics[metadata.speedWindow]` when the key is
///    set on the widget AND the map contains it.
/// 2. `ctx.speed_metrics` (session average) — falls through when the
///    windowed key is missing, so widgets never silently render nothing
///    just because the scanner didn't compute the requested window.
fn pick_metrics<'a>(spec: &WidgetSpec, ctx: &'a RenderContext) -> Option<&'a SpeedMetrics> {
    if let Some(window_key) = spec.metadata.as_ref().and_then(|m| m.get("speedWindow"))
        && let Some(map) = ctx.windowed_speed_metrics.as_ref()
        && let Some(m) = map.get(window_key)
    {
        return Some(m);
    }
    ctx.speed_metrics.as_ref()
}

pub fn input_factory() -> Box<dyn Widget> {
    Box::new(SpeedWidget(Kind::Input))
}
pub fn output_factory() -> Box<dyn Widget> {
    Box::new(SpeedWidget(Kind::Output))
}
pub fn total_factory() -> Box<dyn Widget> {
    Box::new(SpeedWidget(Kind::Total))
}

/// Port of TS `formatSpeed`: `"42.5 t/s"` in the normal range, `"1.2k t/s"`
/// once tokens/sec hits four figures, `"—"` (em-dash) when duration is zero.
fn format_speed(tokens_per_sec: Option<f64>) -> String {
    let Some(v) = tokens_per_sec else {
        return "\u{2014}".to_string();
    };
    if v >= 1_000.0 {
        format!("{:.1}k t/s", v / 1000.0)
    } else {
        format!("{v:.1} t/s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::render_context::SpeedMetrics;

    fn ctx(metrics: SpeedMetrics) -> RenderContext {
        RenderContext {
            speed_metrics: Some(metrics),
            ..Default::default()
        }
    }

    #[test]
    fn input_speed_computes_per_sec_from_metrics() {
        // 1000 input tokens over 10 seconds = 100 t/s.
        let m = SpeedMetrics {
            total_duration_ms: 10_000,
            input_tokens: 1_000,
            output_tokens: 100,
            request_count: 5,
        };
        let spans = input_factory().render(&WidgetSpec::new("1", "input-speed"), &ctx(m));
        assert_eq!(spans[0].text, "In: 100.0 t/s");
    }

    #[test]
    fn output_speed_uses_output_tokens() {
        let m = SpeedMetrics {
            total_duration_ms: 4_000,
            input_tokens: 1_000,
            output_tokens: 200,
            request_count: 1,
        };
        let spans = output_factory().render(&WidgetSpec::new("1", "output-speed"), &ctx(m));
        assert_eq!(spans[0].text, "Out: 50.0 t/s");
    }

    #[test]
    fn total_speed_sums_before_dividing() {
        let m = SpeedMetrics {
            total_duration_ms: 1_000,
            input_tokens: 300,
            output_tokens: 200,
            request_count: 1,
        };
        let spans = total_factory().render(&WidgetSpec::new("1", "total-speed"), &ctx(m));
        assert_eq!(spans[0].text, "Total: 500.0 t/s");
    }

    #[test]
    fn zero_duration_renders_em_dash() {
        let m = SpeedMetrics::default();
        let spans = input_factory().render(&WidgetSpec::new("1", "input-speed"), &ctx(m));
        assert_eq!(spans[0].text, "In: \u{2014}");
    }

    #[test]
    fn kilo_form_kicks_in_at_1000() {
        let m = SpeedMetrics {
            total_duration_ms: 1_000,
            input_tokens: 1_500,
            output_tokens: 0,
            request_count: 1,
        };
        let spans = input_factory().render(&WidgetSpec::new("1", "input-speed"), &ctx(m));
        assert_eq!(spans[0].text, "In: 1.5k t/s");
    }

    #[test]
    fn empty_when_metrics_absent() {
        let spans = input_factory().render(
            &WidgetSpec::new("1", "input-speed"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
