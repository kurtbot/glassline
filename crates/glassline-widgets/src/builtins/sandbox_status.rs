//! `sandbox-status` — whether Claude Code's bash sandbox mode is enabled.
//! Port of upstream `SandboxStatus.ts`.
//!
//! Reads `sandbox.enabled` from Claude Code's layered settings stack (see
//! [`crate::claude_settings`]). When no candidate file exists (Claude
//! Code never initialised), the widget hides. When files exist but no
//! explicit override is set, the value defaults to `false` — matching
//! Claude Code's own default per
//! https://code.claude.com/docs/en/settings#sandbox.
//!
//! Three format variants via `WidgetSpec.metadata.format`:
//! - `glyph` (default): `SB: ●` / `SB: ○` (or the nerd-font lock glyphs).
//! - `text`: `SB: ON` / `SB: OFF`.
//! - `word`: `Sandbox: ON` / `Sandbox: OFF`.
//!
//! Nerd-font glyphs are opt-in via `metadata.useNerdFont = "true"` and
//! only apply in the `glyph` format.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    claude_settings::{read_layered_bool, resolve_claude_config_cwd},
    common::{is_raw, styled},
};

const DOT_ON: &str = "\u{25CF}"; // ●
const DOT_OFF: &str = "\u{25CB}"; // ○
const LOCK_NF: &str = "\u{f023}"; // nerd-font lock
const UNLOCK_NF: &str = "\u{f09c}"; // nerd-font unlock

pub fn factory() -> Box<dyn Widget> {
    Box::new(SandboxStatus)
}

pub struct SandboxStatus;

impl Widget for SandboxStatus {
    fn id(&self) -> &'static str {
        "sandbox-status"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("green")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(cwd) = resolve_claude_config_cwd(ctx) else {
            return Vec::new();
        };
        let Some(result) = read_layered_bool(&cwd, &["sandbox", "enabled"]) else {
            return Vec::new();
        };
        let enabled = result.enabled();
        let text = format_status(enabled, format(spec), use_nerd_font(spec), is_raw(spec));
        styled(spec, text)
    }
}

fn format(spec: &WidgetSpec) -> Format {
    match spec
        .metadata
        .as_ref()
        .and_then(|m| m.get("format"))
        .map(String::as_str)
    {
        Some("text") => Format::Text,
        Some("word") => Format::Word,
        // Anything else (including `"glyph"` and unset) falls to the
        // default — matches upstream's forward-compat behaviour.
        _ => Format::Glyph,
    }
}

fn use_nerd_font(spec: &WidgetSpec) -> bool {
    spec.metadata
        .as_ref()
        .and_then(|m| m.get("useNerdFont"))
        .is_some_and(|v| v == "true")
        // Nerd-font glyphs only apply in glyph format.
        && matches!(format(spec), Format::Glyph)
}

#[derive(Debug, Clone, Copy)]
enum Format {
    Glyph,
    Text,
    Word,
}

fn format_status(enabled: bool, format: Format, nerd_font: bool, raw: bool) -> String {
    let state_text = if enabled { "ON" } else { "OFF" };
    let glyph = if nerd_font {
        if enabled { LOCK_NF } else { UNLOCK_NF }
    } else if enabled {
        DOT_ON
    } else {
        DOT_OFF
    };
    match format {
        Format::Glyph => {
            if raw {
                glyph.to_string()
            } else {
                format!("SB: {glyph}")
            }
        }
        Format::Text => {
            if raw {
                state_text.to_string()
            } else {
                format!("SB: {state_text}")
            }
        }
        Format::Word => {
            if raw {
                state_text.to_string()
            } else {
                format!("Sandbox: {state_text}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_default_labelled() {
        assert_eq!(format_status(true, Format::Glyph, false, false), "SB: \u{25CF}");
        assert_eq!(format_status(false, Format::Glyph, false, false), "SB: \u{25CB}");
    }

    #[test]
    fn glyph_raw_drops_label() {
        assert_eq!(format_status(true, Format::Glyph, false, true), "\u{25CF}");
    }

    #[test]
    fn text_format_uses_on_off() {
        assert_eq!(format_status(true, Format::Text, false, false), "SB: ON");
        assert_eq!(format_status(false, Format::Text, false, false), "SB: OFF");
    }

    #[test]
    fn word_format_spelled_out() {
        assert_eq!(format_status(true, Format::Word, false, false), "Sandbox: ON");
        assert_eq!(
            format_status(false, Format::Word, false, false),
            "Sandbox: OFF"
        );
    }

    #[test]
    fn nerd_font_swaps_glyph_in_glyph_format() {
        assert_eq!(format_status(true, Format::Glyph, true, false), "SB: \u{f023}");
        assert_eq!(
            format_status(false, Format::Glyph, true, false),
            "SB: \u{f09c}"
        );
    }
}
