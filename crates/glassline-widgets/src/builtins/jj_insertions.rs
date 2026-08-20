//! `jj-insertions` — total `+N` insertions from `jj diff --stat`. Port
//! of upstream `JjInsertions.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    jj::{get_jj_diff_stat, no_jj_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(JjInsertions)
}

pub struct JjInsertions;

impl Widget for JjInsertions {
    fn id(&self) -> &'static str {
        "jj-insertions"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let n = get_jj_diff_stat(ctx).insertions;
        if n == 0 {
            return Vec::new();
        }
        styled(spec, format!("+{n}"))
    }
}
