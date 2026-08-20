//! `git-untracked` — glyph if there are untracked files. Port of upstream
//! `GitUntracked.ts`. Default glyph: `?`. Override via `metadata.symbol`.

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
    Box::new(GitUntracked)
}

pub struct GitUntracked;

impl Widget for GitUntracked {
    fn id(&self) -> &'static str {
        "git-untracked"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        if !get_git_status(ctx).untracked {
            return Vec::new();
        }
        let symbol = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("symbol"))
            .cloned()
            .unwrap_or_else(|| "?".to_string());
        styled(spec, symbol)
    }
}
