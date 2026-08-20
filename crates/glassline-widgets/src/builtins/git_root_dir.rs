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
    git::{get_git_root, is_inside_git_work_tree},
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
