//! `free-memory` — available system RAM. Port of upstream `FreeMemory.ts`.
//!
//! Gated on the `sysinfo-widgets` feature (enabled by default). When the
//! feature is off the widget still registers so `settings.json` doesn't
//! throw `[Unknown widget]`, but it renders nothing. See
//! [[widget_parity_design_v1.1]] §12-C1 for the API choice: use
//! `System::new()` with `refresh_memory()` and read `available_memory()`
//! (matches TS `os.freemem()` on Linux — MemAvailable-like, not raw MemFree).

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{labeled_or_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(FreeMemory)
}

pub struct FreeMemory;

impl Widget for FreeMemory {
    fn id(&self) -> &'static str {
        "free-memory"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    #[cfg(feature = "sysinfo-widgets")]
    fn render(&self, spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        use sysinfo::System;
        let mut system = System::new();
        // `refresh_memory` populates RAM + swap. Selective RAM-only refresh
        // (`refresh_memory_specifics`) has a churning API across sysinfo
        // versions (0.30 renamed, 0.32 renamed again); the general call is
        // stable and the extra swap read is a single kernel syscall.
        system.refresh_memory();
        let bytes = system.available_memory();
        if bytes == 0 {
            return Vec::new();
        }
        styled(spec, labeled_or_raw(spec, "Free: ", &format_bytes(bytes)))
    }

    #[cfg(not(feature = "sysinfo-widgets"))]
    fn render(&self, _spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        Vec::new()
    }
}

/// Format a byte count as `N.NG`/`N.NM`/`N.NK`/`N B`. Uses base-2 (1024).
#[cfg(feature = "sysinfo-widgets")]
fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1}G", b / GIB)
    } else if b >= MIB {
        format!("{:.1}M", b / MIB)
    } else if b >= KIB {
        format!("{:.1}K", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(all(test, feature = "sysinfo-widgets"))]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_units() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0K");
        assert_eq!(format_bytes(1024 * 1024 * 3), "3.0M");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 4), "4.0G");
    }

    #[test]
    fn render_produces_something_on_healthy_machine() {
        // Sanity: on any CI runner sysinfo should report >0 available memory.
        let out = FreeMemory.render(
            &WidgetSpec::new("1", "free-memory"),
            &RenderContext::default(),
        );
        assert_eq!(out.len(), 1);
        assert!(out[0].text.starts_with("Free: "));
    }
}
