//! `git-is-fork` — glyph if the local repo is a fork (both `origin` and
//! `upstream` remotes exist AND point to different URLs). Port of
//! upstream `GitIsFork.ts`. Default glyph: `fork`. Override via
//! `metadata.symbol`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_remote_url, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitIsFork)
}

pub struct GitIsFork;

impl Widget for GitIsFork {
    fn id(&self) -> &'static str {
        "git-is-fork"
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
        let origin = get_git_remote_url(ctx, "origin");
        let upstream = get_git_remote_url(ctx, "upstream");
        let (Some(o), Some(u)) = (origin, upstream) else {
            return Vec::new();
        };
        if o == u {
            return Vec::new();
        }
        let symbol = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("symbol"))
            .cloned()
            .unwrap_or_else(|| "fork".to_string());
        styled(spec, symbol)
    }
}
