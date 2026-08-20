//! `jj-description` — description of the current change (`@`). Port of
//! upstream `JjDescription.ts`. Renders the first line only; multi-line
//! descriptions get their body truncated to the summary line.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    jj::{get_jj_description, no_jj_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(JjDescription)
}

pub struct JjDescription;

impl Widget for JjDescription {
    fn id(&self) -> &'static str {
        "jj-description"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("white")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let Some(desc) = get_jj_description(ctx) else {
            return Vec::new();
        };
        // Only the summary line — jj descriptions can be multi-paragraph,
        // and a status line is one line.
        let summary = desc.lines().next().unwrap_or(&desc).trim();
        if summary.is_empty() {
            return Vec::new();
        }
        styled(spec, summary.to_string())
    }
}
