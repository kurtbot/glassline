//! `jj-workspace` — current workspace name (usually `default` unless the
//! user runs multiple workspaces). Port of upstream `JjWorkspace.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    jj::{get_jj_workspace, no_jj_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(JjWorkspace)
}

pub struct JjWorkspace;

impl Widget for JjWorkspace {
    fn id(&self) -> &'static str {
        "jj-workspace"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let Some(name) = get_jj_workspace(ctx) else {
            return Vec::new();
        };
        styled(spec, name)
    }
}
