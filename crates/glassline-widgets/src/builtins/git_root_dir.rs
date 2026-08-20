//! `git-root-dir` — the basename of the git repo root. Port of TS
//! `GitRootDir.tsx`. MVP: no IDE-link hyperlink, no max-width truncation.

use std::path::Path;

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_root, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitRootDir)
}

pub struct GitRootDir;

impl Widget for GitRootDir {
    fn id(&self) -> &'static str {
        "git-root-dir"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let Some(root) = get_git_root(ctx) else {
            return Vec::new();
        };
        let basename = Path::new(&root)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&root);
        styled(spec, basename.to_string())
    }
}
