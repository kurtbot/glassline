//! `git-status` — one-glyph indicators for staged/unstaged/untracked/conflicts.
//! Port of TS `GitStatus.ts`. Default glyphs: `+ * ? !`.
//!
//! **Clean vs failed:** TS returns `null` (empty) both when the tree is
//! clean AND when `git status` fails. We match that behavior — a git
//! failure is indistinguishable from a clean tree, which is fine because
//! neither case has anything actionable to show. See

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
    Box::new(GitStatusW)
}

pub struct GitStatusW;

impl Widget for GitStatusW {
    fn id(&self) -> &'static str {
        "git-status"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("yellow")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let status = get_git_status(ctx);
        let sym_conflicts = symbol(spec, "symbolConflicts", "!");
        let sym_staged = symbol(spec, "symbolStaged", "+");
        let sym_unstaged = symbol(spec, "symbolUnstaged", "*");
        let sym_untracked = symbol(spec, "symbolUntracked", "?");
        let mut out = String::new();
        if status.conflicts {
            out.push_str(&sym_conflicts);
        }
        if status.staged {
            out.push_str(&sym_staged);
        }
        if status.unstaged {
            out.push_str(&sym_unstaged);
        }
        if status.untracked {
            out.push_str(&sym_untracked);
        }
        // Clean tree OR git failure → render nothing. Matches TS
        // `GitStatus.ts` which returns `null` in both cases.
        if out.is_empty() {
            return Vec::new();
        }
        styled(spec, out)
    }
}

fn symbol(spec: &WidgetSpec, key: &str, default: &str) -> String {
    spec.metadata
        .as_ref()
        .and_then(|m| m.get(key))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::StatusJson;

    fn ctx_with_cwd(cwd: &str) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                cwd: Some(cwd.into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn no_repo_shows_no_git_placeholder() {
        // Path that definitely isn't a git repo → widget prints "(no git)".
        let ctx = ctx_with_cwd("C:\\ThisPathDefinitelyDoesNotExist_glassline");
        let spans = GitStatusW.render(&WidgetSpec::new("1", "git-status"), &ctx);
        // Depending on the test host git may or may not be installed; the
        // is_inside_git_work_tree probe returns false either way for a
        // nonexistent cwd, so we expect "(no git)".
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "(no git)");
    }

    #[test]
    fn no_repo_with_hide_no_git_shows_nothing() {
        let mut spec = WidgetSpec::new("1", "git-status");
        spec.metadata = Some(
            [("hideNoGit".to_string(), "true".to_string())]
                .into_iter()
                .collect(),
        );
        let ctx = ctx_with_cwd("C:\\ThisPathDefinitelyDoesNotExist_glassline");
        let spans = GitStatusW.render(&spec, &ctx);
        assert!(spans.is_empty());
    }
}
