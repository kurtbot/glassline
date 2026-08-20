//! `git-ahead-behind` — `↑ahead ↓behind` relative to the tracking branch.
//! Port of upstream `GitAheadBehind.ts`.
//!
//! Empty output when both are zero (in-sync). Empty on missing upstream.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_ahead_behind, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitAheadBehind)
}

pub struct GitAheadBehind;

impl Widget for GitAheadBehind {
    fn id(&self) -> &'static str {
        "git-ahead-behind"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let Some((ahead, behind)) = get_git_ahead_behind(ctx) else {
            return Vec::new();
        };
        let mut parts: Vec<String> = Vec::new();
        if ahead > 0 {
            parts.push(format!("\u{2191}{ahead}"));
        }
        if behind > 0 {
            parts.push(format!("\u{2193}{behind}"));
        }
        if parts.is_empty() {
            return Vec::new();
        }
        styled(spec, parts.join(" "))
    }
}
