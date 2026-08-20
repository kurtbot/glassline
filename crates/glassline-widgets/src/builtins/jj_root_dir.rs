//! `jj-root-dir` — basename of the jj workspace root via `jj root`.
//! Port of upstream `JjRootDir.ts`.

use std::path::Path;

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    jj::{get_jj_root, no_jj_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(JjRootDir)
}

pub struct JjRootDir;

impl Widget for JjRootDir {
    fn id(&self) -> &'static str {
        "jj-root-dir"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("cyan")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_jj_short_circuit(spec, ctx) {
            return early;
        }
        let Some(root) = get_jj_root(ctx) else {
            return Vec::new();
        };
        let basename = Path::new(&root)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&root);
        styled(spec, basename.to_string())
    }
}
