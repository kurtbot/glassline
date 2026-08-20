//! `git-origin-owner-repo` — `owner/repo` of the `origin` remote. Port
//! of upstream `GitOriginOwnerRepo.ts`.

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
    Box::new(GitOriginOwnerRepo)
}

pub struct GitOriginOwnerRepo;

impl Widget for GitOriginOwnerRepo {
    fn id(&self) -> &'static str {
        "git-origin-owner-repo"
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
        let Some(remote) = get_git_remote(ctx, "origin") else {
            return Vec::new();
        };
        styled(spec, format!("{}/{}", remote.owner, remote.repo))
    }
}
