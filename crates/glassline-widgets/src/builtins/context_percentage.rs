//! `context-percentage` — percentage of the context window used (or
//! remaining). Port of TS `ContextPercentage.ts`.
//!
//! **Deferred:** the "slider" progress bar mode + per-widget "inverse"
//! toggle land in T-1.7b. MVP renders `Ctx Used: 9.3%` / `9.3%` raw.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{context_window_metrics, default_context_window_size, is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ContextPercentage)
}

pub struct ContextPercentage;

impl Widget for ContextPercentage {
    fn id(&self) -> &'static str {
        "context-percentage"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let metrics = context_window_metrics(ctx.data.as_ref());
        let inverse = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("inverse"))
            .is_some_and(|v| v == "true");

        let used_pct = if let Some(pct) = metrics.used_percentage {
            pct
        } else if let Some(tokens) = ctx.token_metrics.as_ref().map(|m| m.context_length) {
            let window = metrics
                .window_size
                .unwrap_or_else(default_context_window_size);
            if window == 0 {
                return Vec::new();
            }
            (tokens as f64 / window as f64 * 100.0).min(100.0)
        } else {
            return Vec::new();
        };

        let display_pct = if inverse { 100.0 - used_pct } else { used_pct };
        let label = if inverse { "Ctx Left: " } else { "Ctx Used: " };
        let formatted = format!("{display_pct:.1}%");
        let text = if is_raw(spec) {
            formatted
        } else {
            format!("{label}{formatted}")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::{
        render_context::TokenMetrics,
        status_json::{ContextWindow, CurrentUsage, StatusJson},
    };

    #[test]
    fn used_percentage_from_status_json() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    used_percentage: Some(9.3),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextPercentage.render(&WidgetSpec::new("1", "context-percentage"), &ctx);
        assert_eq!(spans[0].text, "Ctx Used: 9.3%");
    }

    #[test]
    fn inverse_metadata_flips_label_and_value() {
        let mut spec = WidgetSpec::new("1", "context-percentage");
        spec.metadata = Some(
            [("inverse".to_string(), "true".to_string())]
                .into_iter()
                .collect(),
        );
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    used_percentage: Some(30.0),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextPercentage.render(&spec, &ctx);
        assert_eq!(spans[0].text, "Ctx Left: 70.0%");
    }

    #[test]
    fn falls_back_to_transcript_over_default_window() {
        let ctx = RenderContext {
            token_metrics: Some(TokenMetrics {
                context_length: 20_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextPercentage.render(&WidgetSpec::new("1", "context-percentage"), &ctx);
        // 20_000 / 200_000 * 100 = 10.0%
        assert_eq!(spans[0].text, "Ctx Used: 10.0%");
    }

    #[test]
    fn falls_back_uses_status_json_window_size_when_present() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    context_window_size: Some(100_000.0),
                    current_usage: Some(CurrentUsage::Breakdown {
                        input_tokens: Some(25_000.0),
                        output_tokens: None,
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: Some(10_000.0),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextPercentage.render(&WidgetSpec::new("1", "context-percentage"), &ctx);
        // current_usage sum = 35_000 (input+read); window = 100k -> 35%
        assert_eq!(spans[0].text, "Ctx Used: 35.0%");
    }

    #[test]
    fn raw_drops_label() {
        let mut spec = WidgetSpec::new("1", "context-percentage");
        spec.raw_value = Some(true);
        let ctx = RenderContext {
            data: Some(StatusJson {
                context_window: Some(ContextWindow {
                    used_percentage: Some(42.5),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = ContextPercentage.render(&spec, &ctx);
        assert_eq!(spans[0].text, "42.5%");
    }

    #[test]
    fn empty_when_nothing_available() {
        let spans = ContextPercentage.render(
            &WidgetSpec::new("1", "context-percentage"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
