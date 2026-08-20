//! `git-worktree-original-branch` — the branch the main worktree had
//! before this linked worktree was created. Reads
//! `StatusJson.worktree.original_branch`. Port of upstream
//! `GitWorktreeOriginalBranch.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitWorktreeOriginalBranch)
}

pub struct GitWorktreeOriginalBranch;

impl Widget for GitWorktreeOriginalBranch {
    fn id(&self) -> &'static str {
        "git-worktree-original-branch"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(branch) = ctx
            .data
            .as_ref()
            .and_then(|d| d.worktree.as_ref())
            .and_then(|w| w.original_branch.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        styled(spec, branch.to_string())
    }
}
