//! `git-staged` — glyph if there are staged changes. Port of upstream
//! `GitStaged.ts`. Default glyph: `+`. Override via `metadata.symbol`.

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
    Box::new(GitStaged)
}

pub struct GitStaged;

impl Widget for GitStaged {
    fn id(&self) -> &'static str {
        "git-staged"
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
        if !get_git_status(ctx).staged {
            return Vec::new();
        }
        let symbol = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("symbol"))
            .cloned()
            .unwrap_or_else(|| "+".to_string());
        styled(spec, symbol)
    }
}
