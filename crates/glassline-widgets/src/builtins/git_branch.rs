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
    git::{get_git_branch, is_inside_git_work_tree},
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
        let text = get_git_branch(ctx).unwrap_or_else(|| "(detached)".to_string());
        styled(spec, text)
    }
}
