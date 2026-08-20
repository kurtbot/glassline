//! `jj-bookmarks` — bookmarks pointing at the current change (`@`).
//! Port of upstream `JjBookmarks.ts`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    jj::{get_jj_bookmarks, no_jj_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(JjBookmarks)
}

pub struct JjBookmarks;

impl Widget for JjBookmarks {
    fn id(&self) -> &'static str {
        "jj-bookmarks"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let Some(names) = get_jj_bookmarks(ctx) else {
            return Vec::new();
        };
        styled(spec, names)
    }
}
