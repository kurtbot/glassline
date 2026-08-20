//! `git-worktree-name` — the linked worktree's name from
//! `StatusJson.worktree.name`. Port of upstream `GitWorktreeName.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitWorktreeName)
}

pub struct GitWorktreeName;

impl Widget for GitWorktreeName {
    fn id(&self) -> &'static str {
        "git-worktree-name"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(name) = ctx
            .data
            .as_ref()
            .and_then(|d| d.worktree.as_ref())
            .and_then(|w| w.name.as_deref())
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        styled(spec, name.to_string())
    }
}
