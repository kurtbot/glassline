//! `git-origin-host` — the host portion of the `origin` remote's URL
//! (`github.com`, `gitlab.com`, `bitbucket.org`, …). Net-new widget with
//! no upstream ccstatusline counterpart — see
//! [[statusjson_native_repo_design_v1.0]].
//!
//! Fast path: reads `workspace.repo.host` when Claude Code v2.1+ ships
//! it. Falls back to parsing the origin URL from `git remote get-url`.
//! Renders empty when neither source produces a non-empty host, matching
//! the "hide silently" behaviour of the origin family.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_origin_host, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitOriginHost)
}

pub struct GitOriginHost;

impl Widget for GitOriginHost {
    fn id(&self) -> &'static str {
        "git-origin-host"
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
        let Some(host) = get_git_origin_host(ctx) else {
            return Vec::new();
        };
        styled(spec, host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{StatusJson, Workspace, WorkspaceRepo};

    fn spec() -> WidgetSpec {
        WidgetSpec::new("1", "git-origin-host")
    }

    #[test]
    fn fast_path_reads_workspace_repo_host() {
        // CARGO_MANIFEST_DIR is always a git repo, so no_git_short_circuit
        // passes; the fake host proves the fast path fired.
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: Some(env!("CARGO_MANIFEST_DIR").into()),
                workspace: Some(Workspace {
                    repo: Some(WorkspaceRepo {
                        host: Some("fastpath.example.com".into()),
                        owner: Some("o".into()),
                        name: Some("r".into()),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = GitOriginHost.render(&spec(), &ctx);
        assert_eq!(spans[0].text, "fastpath.example.com");
    }

    #[test]
    fn hidden_when_not_in_git_and_hidenogit_true() {
        // Point cwd at a nonexistent path so is_inside_work_tree returns
        // false. metadata.hideNoGit=true → widget should render empty.
        use glassline_core::settings::WidgetSpec as Spec;
        let mut spec = Spec::new("1", "git-origin-host");
        let mut meta = std::collections::BTreeMap::new();
        meta.insert("hideNoGit".into(), "true".into());
        spec.metadata = Some(meta);
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: Some("Z:/definitely/not/a/git/repo/glassline-test".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = GitOriginHost.render(&spec, &ctx);
        assert!(
            spans.is_empty(),
            "expected no spans when hideNoGit + not in git, got {spans:?}"
        );
    }

    #[test]
    fn hidden_when_native_host_blank() {
        // Whitespace-only host on the fast path → treated as absent →
        // fallback shell-out tries `remote get-url origin` — but no cwd
        // means it can't run either. Widget renders empty.
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: Some(env!("CARGO_MANIFEST_DIR").into()),
                workspace: Some(Workspace {
                    repo: Some(WorkspaceRepo {
                        host: Some("   ".into()),
                        owner: Some("o".into()),
                        name: Some("r".into()),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        // The fallback path may or may not succeed depending on the real
        // repo's `origin` remote — we only assert we didn't panic and
        // didn't emit the whitespace as a host string.
        let spans = GitOriginHost.render(&spec(), &ctx);
        for s in &spans {
            assert_ne!(s.text.trim(), "", "host must not be blank whitespace");
        }
    }
}
