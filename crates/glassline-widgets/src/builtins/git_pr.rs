//! `git-pr` — the pull request for the current branch, formatted as
//! `#123` (or empty when there's no PR). Port of upstream `GitPr.ts`.
//!
//! Uses `gh pr view --json state,number,title` per
//! Empty when `gh` is absent,
//! unauthenticated, or the branch has no PR.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{gh_available, no_git_short_circuit, run_gh},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitPr)
}

pub struct GitPr;

impl Widget for GitPr {
    fn id(&self) -> &'static str {
        "git-pr"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightCyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        if !gh_available() {
            return Vec::new();
        }
        let raw = run_gh(&["pr", "view", "--json", "state,number,title"], ctx);
        let Some(json) = raw else { return Vec::new() };
        let parsed: serde_json::Value = match serde_json::from_str(&json) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        let number = parsed.get("number").and_then(|v| v.as_u64());
        let Some(n) = number else {
            return Vec::new();
        };
        let state = parsed.get("state").and_then(|v| v.as_str()).unwrap_or("");
        let mut out = format!("#{n}");
        if !state.is_empty() {
            out.push_str(&format!(" ({})", state.to_lowercase()));
        }
        styled(spec, out)
    }
}
