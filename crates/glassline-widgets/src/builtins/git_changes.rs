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
    git::{get_git_change_counts, is_inside_git_work_tree},
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
        let hide_no_git = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("hideNoGit"))
            .is_some_and(|v| v == "true");
        if !is_inside_git_work_tree(ctx) {
            return if hide_no_git {
                Vec::new()
            } else {
                styled(spec, "(no git)".into())
            };
        }
        let counts = get_git_change_counts(ctx);
        styled(
            spec,
            format!("(+{},-{})", counts.insertions, counts.deletions),
        )
    }
}
