//! `git-staged-files` — count of staged entries. Port of upstream
//! `GitStagedFiles.ts`.

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
    Box::new(GitStagedFiles)
}

pub struct GitStagedFiles;

impl Widget for GitStagedFiles {
    fn id(&self) -> &'static str {
        "git-staged-files"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let n = get_git_status(ctx).staged_files;
        if n == 0 {
            return Vec::new();
        }
        styled(spec, n.to_string())
    }
}
