//! `remote-control-status` — whether Claude Code's remote-control is
//! attached to the current session. Port of upstream
//! `RemoteControlStatus.ts`.
//!
//! Reads `remoteControl.enabled` from the per-session JSON file under
//! `$CLAUDE_CONFIG_DIR/sessions/*.json` (or `~/.claude/sessions/*.json`)
//! whose top-level `sessionId` matches `StatusJson.session_id`. Returns
//! `Vec::new()` when no session_id is present or no matching file is
//! found — the same "hide silently" contract the sandbox/voice widgets
//! use.
//!
//! Six format variants via `WidgetSpec.metadata.format`:
//! - `icon` (default): `📡 ◉` / `📡 ○`; nerd-font collapses to a single glyph.
//! - `icon-text`: `📡 on` / `📡 off`.
//! - `text`: bare `on` / `off`.
//! - `word`: `remote on` / `remote off`.
//! - `label-check`: `remote ✅` / `remote ❌`.
//! - `label-mark`: `remote ✓` / `remote ✗`.
//!
//! Nerd-font opt-in via `metadata.useNerdFont = "true"` (icon and
//! icon-text formats only).

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    claude_settings::read_remote_control_status,
    common::{is_raw, styled},
};

const SATELLITE_EMOJI: &str = "\u{1f4e1}"; // 📡
const SATELLITE_NF: &str = "\u{f7c0}"; // nerd-font satellite
const SATELLITE_SLASH_NF: &str = "\u{f4b5}"; // nerd-font satellite slash
const STATE_DOT_ON: &str = "\u{25c9}"; // ◉
const STATE_DOT_OFF: &str = "\u{25cb}"; // ○
const CHECK_EMOJI: &str = "\u{2705}"; // ✅
const CROSS_EMOJI: &str = "\u{274c}"; // ❌
const CHECK_MARK: &str = "\u{2713}"; // ✓
const CROSS_MARK: &str = "\u{2717}"; // ✗

pub fn factory() -> Box<dyn Widget> {
    Box::new(RemoteControlStatus)
}

pub struct RemoteControlStatus;

impl Widget for RemoteControlStatus {
    fn id(&self) -> &'static str {
        "remote-control-status"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let session_id = ctx
            .data
            .as_ref()
            .and_then(|d| d.session_id.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(session_id) = session_id else {
            return Vec::new();
        };
        let Some(enabled) = read_remote_control_status(session_id) else {
            return Vec::new();
        };
        let text = format_status(enabled, format(spec), use_nerd_font(spec), is_raw(spec));
        styled(spec, text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Icon,
    IconText,
    Text,
    Word,
    LabelCheck,
    LabelMark,
}

fn format(spec: &WidgetSpec) -> Format {
    match spec
        .metadata
        .as_ref()
        .and_then(|m| m.get("format"))
        .map(String::as_str)
    {
        Some("icon-text") => Format::IconText,
        Some("text") => Format::Text,
        Some("word") => Format::Word,
        Some("label-check") => Format::LabelCheck,
        Some("label-mark") => Format::LabelMark,
        _ => Format::Icon,
    }
}

fn use_nerd_font(spec: &WidgetSpec) -> bool {
    spec.metadata
        .as_ref()
        .and_then(|m| m.get("useNerdFont"))
        .is_some_and(|v| v == "true")
        && matches!(format(spec), Format::Icon | Format::IconText)
}

fn format_status(enabled: bool, format: Format, nerd_font: bool, raw: bool) -> String {
    let state_text = if enabled { "on" } else { "off" };
    let state_dot = if enabled { STATE_DOT_ON } else { STATE_DOT_OFF };
    let icon = if nerd_font {
        if enabled {
            SATELLITE_NF
        } else {
            SATELLITE_SLASH_NF
        }
    } else {
        SATELLITE_EMOJI
    };
    match format {
        Format::Icon => {
            if nerd_font {
                icon.to_string()
            } else if raw {
                state_dot.to_string()
            } else {
                format!("{icon} {state_dot}")
            }
        }
        Format::IconText => {
            if raw {
                state_text.to_string()
            } else {
                format!("{icon} {state_text}")
            }
        }
        Format::Text => state_text.to_string(),
        Format::Word => {
            if raw {
                state_text.to_string()
            } else {
                format!("remote {state_text}")
            }
        }
        Format::LabelCheck => {
            let mark = if enabled { CHECK_EMOJI } else { CROSS_EMOJI };
            if raw {
                mark.to_string()
            } else {
                format!("remote {mark}")
            }
        }
        Format::LabelMark => {
            let mark = if enabled { CHECK_MARK } else { CROSS_MARK };
            if raw {
                mark.to_string()
            } else {
                format!("remote {mark}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_default_shows_satellite_plus_dot() {
        assert_eq!(
            format_status(true, Format::Icon, false, false),
            "\u{1f4e1} \u{25c9}"
        );
    }

    #[test]
    fn icon_nerd_font_collapses_to_glyph() {
        assert_eq!(format_status(true, Format::Icon, true, false), "\u{f7c0}");
        assert_eq!(format_status(false, Format::Icon, true, false), "\u{f4b5}");
    }

    #[test]
    fn icon_text_labelled() {
        assert_eq!(
            format_status(true, Format::IconText, false, false),
            "\u{1f4e1} on"
        );
    }

    #[test]
    fn text_format_bare() {
        assert_eq!(format_status(true, Format::Text, false, false), "on");
        assert_eq!(format_status(false, Format::Text, false, false), "off");
    }

    #[test]
    fn word_format_prefixed() {
        assert_eq!(format_status(true, Format::Word, false, false), "remote on");
    }

    #[test]
    fn label_check_uses_emoji() {
        assert_eq!(
            format_status(true, Format::LabelCheck, false, false),
            "remote \u{2705}"
        );
        assert_eq!(
            format_status(false, Format::LabelCheck, false, false),
            "remote \u{274c}"
        );
    }

    #[test]
    fn label_mark_uses_ascii_check() {
        assert_eq!(
            format_status(true, Format::LabelMark, false, false),
            "remote \u{2713}"
        );
        assert_eq!(
            format_status(false, Format::LabelMark, false, false),
            "remote \u{2717}"
        );
    }

    #[test]
    fn label_check_raw_drops_prefix() {
        assert_eq!(
            format_status(true, Format::LabelCheck, false, true),
            "\u{2705}"
        );
    }
}
