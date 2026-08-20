//! `git-sha` — short commit hash. Port of TS `GitSha.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_short_sha, no_git_short_circuit},
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
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        // For the in-repo-but-no-commit case we honor `hideNoGit` too —
        // the flag semantically covers "git isn't showing anything useful".
        let hide = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("hideNoGit"))
            .is_some_and(|v| v == "true");
        let text = get_git_short_sha(ctx).unwrap_or_else(|| {
            if hide {
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
