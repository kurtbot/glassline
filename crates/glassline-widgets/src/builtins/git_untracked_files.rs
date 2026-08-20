//! `git-untracked-files` — count of untracked entries. Port of upstream
//! `GitUntrackedFiles.ts`.

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
    Box::new(GitUntrackedFiles)
}

pub struct GitUntrackedFiles;

impl Widget for GitUntrackedFiles {
    fn id(&self) -> &'static str {
        "git-untracked-files"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let n = get_git_status(ctx).untracked_files;
        if n == 0 {
            return Vec::new();
        }
        styled(spec, n.to_string())
    }
}
