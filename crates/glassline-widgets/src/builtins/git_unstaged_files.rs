//! `git-unstaged-files` — count of unstaged-changed entries. Port of
//! upstream `GitUnstagedFiles.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_status, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitUnstagedFiles)
}

pub struct GitUnstagedFiles;

impl Widget for GitUnstagedFiles {
    fn id(&self) -> &'static str {
        "git-unstaged-files"
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
        let n = get_git_status(ctx).unstaged_files;
        if n == 0 {
            return Vec::new();
        }
        styled(spec, n.to_string())
    }
}
