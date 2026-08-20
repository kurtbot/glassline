//! `git-conflicts` — glyph if there are merge conflicts. Port of upstream
//! `GitConflicts.ts`. Default glyph: `!`. Override via `metadata.symbol`.

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
    Box::new(GitConflicts)
}

pub struct GitConflicts;

impl Widget for GitConflicts {
    fn id(&self) -> &'static str {
        "git-conflicts"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("red")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        if !get_git_status(ctx).conflicts {
            return Vec::new();
        }
        let symbol = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("symbol"))
            .cloned()
            .unwrap_or_else(|| "!".to_string());
        styled(spec, symbol)
    }
}
