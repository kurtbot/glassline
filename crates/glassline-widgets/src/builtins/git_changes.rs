//! `git-changes` — `(+insertions,-deletions)` from `git diff --shortstat`.
//! Port of TS `GitChanges.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_change_counts, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitChanges)
}

pub struct GitChanges;

impl Widget for GitChanges {
    fn id(&self) -> &'static str {
        "git-changes"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("yellow")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let counts = get_git_change_counts(ctx);
        styled(
            spec,
            format!("(+{},-{})", counts.insertions, counts.deletions),
        )
    }
}
