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
    git::{get_git_origin, no_git_short_circuit},
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
        let Some(remote) = get_git_origin(ctx) else {
            return Vec::new();
        };
        styled(spec, format!("{}/{}", remote.owner, remote.repo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{StatusJson, Workspace, WorkspaceRepo};

    #[test]
    fn fast_path_joins_workspace_repo_owner_and_name() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: Some(env!("CARGO_MANIFEST_DIR").into()),
                workspace: Some(Workspace {
                    repo: Some(WorkspaceRepo {
                        host: Some("example.com".into()),
                        owner: Some("fastpath-owner".into()),
                        name: Some("fastpath-name".into()),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = GitOriginOwnerRepo.render(&WidgetSpec::new("1", "git-origin-owner-repo"), &ctx);
        assert_eq!(spans[0].text, "fastpath-owner/fastpath-name");
    }
}
