//! `git-origin-owner` — the owner of the `origin` remote (`kurtbot` from
//! `git@github.com:kurtbot/glassline.git`). Port of upstream `GitOriginOwner.ts`.

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
    Box::new(GitOriginOwner)
}

pub struct GitOriginOwner;

impl Widget for GitOriginOwner {
    fn id(&self) -> &'static str {
        "git-origin-owner"
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
        styled(spec, remote.owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{StatusJson, Workspace, WorkspaceRepo};

    #[test]
    fn fast_path_reads_workspace_repo_owner() {
        // Point cwd at this crate root — always a git repo, so
        // `no_git_short_circuit` passes without opting out. The fake
        // owner value proves the fast path fired: `git remote get-url
        // origin` on the real repo would never produce this string.
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
        let spans = GitOriginOwner.render(&WidgetSpec::new("1", "git-origin-owner"), &ctx);
        assert_eq!(spans[0].text, "fastpath-owner");
    }
}
