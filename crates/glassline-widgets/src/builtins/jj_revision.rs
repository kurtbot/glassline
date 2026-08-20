//! `jj-revision` — short commit_id of the current change (`@`). Port of
//! upstream `JjRevision.ts`. Configurable length via `metadata.length`
//! (default 8, capped at 64).

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    jj::{get_jj_revision, no_jj_short_circuit},
};

const DEFAULT_LENGTH: usize = 8;

pub fn factory() -> Box<dyn Widget> {
    Box::new(JjRevision)
}

pub struct JjRevision;

impl Widget for JjRevision {
    fn id(&self) -> &'static str {
        "jj-revision"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let Some(full) = get_jj_revision(ctx) else {
            return Vec::new();
        };
        let n = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("length"))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_LENGTH)
            .min(64);
        let short: String = full.chars().take(n).collect();
        styled(spec, short)
    }
}
