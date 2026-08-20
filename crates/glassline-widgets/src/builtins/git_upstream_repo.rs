//! `git-upstream-repo` — repo name of the `upstream` remote. Port of
//! upstream `GitUpstreamRepo.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_remote, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitUpstreamRepo)
}

pub struct GitUpstreamRepo;

impl Widget for GitUpstreamRepo {
    fn id(&self) -> &'static str {
        "git-upstream-repo"
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
        let Some(remote) = get_git_remote(ctx, "upstream") else {
            return Vec::new();
        };
        styled(spec, remote.repo)
    }
}
