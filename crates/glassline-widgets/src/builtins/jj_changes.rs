//! `jj-changes` — `(+insertions,-deletions)` from `jj diff --stat`.
//! Port of upstream `JjChanges.ts`.

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
    Box::new(JjChanges)
}

pub struct JjChanges;

impl Widget for JjChanges {
    fn id(&self) -> &'static str {
        "jj-changes"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("yellow")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let stat = get_jj_diff_stat(ctx);
        styled(spec, format!("(+{},-{})", stat.insertions, stat.deletions))
    }
}
