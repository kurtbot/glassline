//! `git-worktree` — glyph if the current workspace is a linked worktree
//! (Claude Code populates `StatusJson.worktree` only for linked
//! worktrees, not the main worktree). Port of upstream `GitWorktree.ts`.
//! Default glyph: `worktree`. Override via `metadata.symbol`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitWorktree)
}

pub struct GitWorktree;

impl Widget for GitWorktree {
    fn id(&self) -> &'static str {
        "git-worktree"
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
        let symbol = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("symbol"))
            .cloned()
            .unwrap_or_else(|| "worktree".to_string());
        styled(spec, symbol)
    }
}
