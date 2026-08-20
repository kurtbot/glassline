//! `voice-status` — whether Claude Code's voice-dictation input is on.
//! Port of upstream `VoiceStatus.ts`.
//!
//! Reads `voice.enabled` from Claude Code's layered settings stack (see
//! [`crate::claude_settings`]). Same "hide when no file exists / default
//! to false when files exist without override" semantics as
//! [`crate::builtins::sandbox_status`]. Setting shape confirmed against
//! https://code.claude.com/docs/en/voice-dictation#configure-voice-dictation-in-settings.
//!
//! Four format variants via `WidgetSpec.metadata.format`:
//! - `icon` (default): `🎤 ◉` / `🎤 ○` (or nerd-font mic glyph alone).
//! - `icon-text`: `🎤 on` / `🎤 off`.
//! - `text`: `on` / `off`.
//! - `word`: `voice on` / `voice off`.
//!
//! Nerd-font opt-in via `metadata.useNerdFont = "true"`; applies to
//! `icon` and `icon-text` formats. Raw mode drops verbose prefixes.

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

const MIC_EMOJI: &str = "\u{1f3a4}"; // 🎤
const MIC_NF: &str = "\u{f130}"; // nerd-font microphone
const MIC_SLASH_NF: &str = "\u{f131}"; // nerd-font microphone-slash
const STATE_DOT_ON: &str = "\u{25c9}"; // ◉
const STATE_DOT_OFF: &str = "\u{25cb}"; // ○

pub fn factory() -> Box<dyn Widget> {
    Box::new(VoiceStatus)
}

pub struct VoiceStatus;

impl Widget for VoiceStatus {
    fn id(&self) -> &'static str {
        "voice-status"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(cwd) = resolve_claude_config_cwd(ctx) else {
            return Vec::new();
        };
        let Some(result) = read_layered_bool(&cwd, &["voice", "enabled"]) else {
            return Vec::new();
        };
        let text = format_status(
            result.enabled(),
            format(spec),
            use_nerd_font(spec),
            is_raw(spec),
        );
        styled(spec, text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Icon,
    IconText,
    Text,
    Word,
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
        // Anything else (including `"icon"` and unset) defaults to icon.
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
        if enabled { MIC_NF } else { MIC_SLASH_NF }
    } else {
        MIC_EMOJI
    };
    match format {
        Format::Icon => {
            if nerd_font {
                // Nerd-font mic glyph already conveys on/off — no dot needed.
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
                format!("voice {state_text}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_default_shows_emoji_plus_dot() {
        assert_eq!(
            format_status(true, Format::Icon, false, false),
            "\u{1f3a4} \u{25c9}"
        );
        assert_eq!(
            format_status(false, Format::Icon, false, false),
            "\u{1f3a4} \u{25cb}"
        );
    }

    #[test]
    fn icon_raw_drops_emoji() {
        assert_eq!(format_status(true, Format::Icon, false, true), "\u{25c9}");
    }

    #[test]
    fn icon_nerd_font_shows_mic_glyph_only() {
        assert_eq!(format_status(true, Format::Icon, true, false), "\u{f130}");
        assert_eq!(format_status(false, Format::Icon, true, false), "\u{f131}");
    }

    #[test]
    fn icon_text_labelled() {
        assert_eq!(
            format_status(true, Format::IconText, false, false),
            "\u{1f3a4} on"
        );
        assert_eq!(
            format_status(false, Format::IconText, false, false),
            "\u{1f3a4} off"
        );
    }

    #[test]
    fn icon_text_raw_drops_icon() {
        assert_eq!(format_status(true, Format::IconText, false, true), "on");
    }

    #[test]
    fn text_format_bare() {
        assert_eq!(format_status(true, Format::Text, false, false), "on");
        assert_eq!(format_status(false, Format::Text, false, false), "off");
    }

    #[test]
    fn word_format_prefixed() {
        assert_eq!(format_status(true, Format::Word, false, false), "voice on");
        assert_eq!(format_status(true, Format::Word, false, true), "on");
    }
}
