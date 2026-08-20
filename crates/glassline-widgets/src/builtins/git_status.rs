//! `git-status` — one-glyph indicators for staged/unstaged/untracked/conflicts.
//! Port of TS `GitStatus.ts`. Default glyphs: `+ * ? !`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_status, is_inside_git_work_tree},
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
        if out.is_empty() {
            out.push_str("clean");
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
