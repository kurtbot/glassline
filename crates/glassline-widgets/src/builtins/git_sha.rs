//! `git-sha` — short commit hash. Port of TS `GitSha.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_short_sha, is_inside_git_work_tree},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitSha)
}

pub struct GitSha;

impl Widget for GitSha {
    fn id(&self) -> &'static str {
        "git-sha"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("gray")
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
        let text = get_git_short_sha(ctx).unwrap_or_else(|| {
            if hide_no_git {
                String::new()
            } else {
                "(no commit)".to_string()
            }
        });
        if text.is_empty() {
            return Vec::new();
        }
        styled(spec, text)
    }
}
