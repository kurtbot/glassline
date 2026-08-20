//! `git-worktree-mode` — indicator that this is a linked worktree.
//! Renders `linked` when `StatusJson.worktree` is present, empty otherwise.
//! Port of upstream `GitWorktreeMode.ts`.
//!
//! Upstream distinguishes multiple modes (`bare`, `detached`, `linked`)
//! from `git worktree list` output. Claude Code doesn't currently expose
//! the mode field on `StatusJson`, so we return `linked` for any
//! linked-worktree state — the value users care about matches TS's most
//! common case. Additional modes land alongside a full worktree scanner
//! in a follow-up pass.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitWorktreeMode)
}

pub struct GitWorktreeMode;

impl Widget for GitWorktreeMode {
    fn id(&self) -> &'static str {
        "git-worktree-mode"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if ctx
            .data
            .as_ref()
            .and_then(|d| d.worktree.as_ref())
            .is_none()
        {
            return Vec::new();
        }
        styled(spec, "linked".to_string())
    }
}
