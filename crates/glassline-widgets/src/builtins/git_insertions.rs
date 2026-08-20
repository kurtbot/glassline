//! `git-insertions` — total `+N` insertions from `git diff --shortstat`
//! (staged + unstaged combined). Port of upstream `GitInsertions.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_change_counts, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitInsertions)
}

pub struct GitInsertions;

impl Widget for GitInsertions {
    fn id(&self) -> &'static str {
        "git-insertions"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let n = get_git_change_counts(ctx).insertions;
        if n == 0 {
            return Vec::new();
        }
        styled(spec, format!("+{n}"))
    }
}
