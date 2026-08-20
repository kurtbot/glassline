//! `git-worktree-branch` — the linked worktree's checked-out branch from
//! `StatusJson.worktree.branch`. Port of upstream `GitWorktreeBranch.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitWorktreeBranch)
}

pub struct GitWorktreeBranch;

impl Widget for GitWorktreeBranch {
    fn id(&self) -> &'static str {
        "git-worktree-branch"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(branch) = ctx
            .data
            .as_ref()
            .and_then(|d| d.worktree.as_ref())
            .and_then(|w| w.branch.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        styled(spec, branch.to_string())
    }
}
