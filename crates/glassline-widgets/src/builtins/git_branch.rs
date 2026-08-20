//! `git-branch` — current branch name (or `(no git)` when outside a repo).
//! Port of TS `GitBranch.tsx` render path. MVP: no hyperlink, no max-width
//! truncation, no symbol prefix override.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_branch, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitBranch)
}

pub struct GitBranch;

impl Widget for GitBranch {
    fn id(&self) -> &'static str {
        "git-branch"
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
        let text = get_git_branch(ctx).unwrap_or_else(|| "(detached)".to_string());
        styled(spec, text)
    }
}
