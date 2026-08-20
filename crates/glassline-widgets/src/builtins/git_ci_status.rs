//! `git-ci-status` — glyph for the most recent CI run's conclusion via
//! `gh run list --json`. Port of upstream `GitCiStatus.ts`.
//!
//! Empty when `gh` isn't installed, the user isn't authenticated, or the
//! repo has no runs. Cached per-invocation via `run_gh`.

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
    Box::new(GitCiStatus)
}

pub struct GitCiStatus;

impl Widget for GitCiStatus {
    fn id(&self) -> &'static str {
        "git-ci-status"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        if !gh_available() {
            return Vec::new();
        }
        let raw = run_gh(
            &[
                "run",
                "list",
                "--limit",
                "1",
                "--json",
                "status,conclusion",
                "--branch",
                &current_branch(ctx),
            ],
            ctx,
        );
        let Some(json) = raw else { return Vec::new() };
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap_or_default();
        let Some(entry) = parsed.first() else {
            return Vec::new();
        };
        let conclusion = entry
            .get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let status = entry.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let glyph = pick_glyph(status, conclusion);
        if glyph.is_empty() {
            return Vec::new();
        }
        styled(spec, glyph.to_string())
    }
}

fn pick_glyph(status: &str, conclusion: &str) -> &'static str {
    match conclusion {
        "success" => "\u{2713}",                                                 // ✓
        "failure" | "timed_out" | "cancelled" | "startup_failure" => "\u{2717}", // ✗
        "skipped" | "neutral" | "action_required" => "\u{25CB}",                 // ○
        _ => match status {
            "in_progress" | "queued" | "requested" | "waiting" => "\u{25CB}",
            _ => "",
        },
    }
}

fn current_branch(ctx: &RenderContext) -> String {
    crate::git::get_git_branch(ctx).unwrap_or_else(|| "HEAD".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_success() {
        assert_eq!(pick_glyph("completed", "success"), "\u{2713}");
    }

    #[test]
    fn glyph_failure_family() {
        assert_eq!(pick_glyph("completed", "failure"), "\u{2717}");
        assert_eq!(pick_glyph("completed", "timed_out"), "\u{2717}");
        assert_eq!(pick_glyph("completed", "cancelled"), "\u{2717}");
    }

    #[test]
    fn glyph_pending() {
        assert_eq!(pick_glyph("in_progress", ""), "\u{25CB}");
        assert_eq!(pick_glyph("queued", ""), "\u{25CB}");
    }

    #[test]
    fn glyph_unknown_empty() {
        assert_eq!(pick_glyph("completed", "unknown-conclusion"), "");
        assert_eq!(pick_glyph("", ""), "");
    }
}
