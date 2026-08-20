//! `vim-mode` — current vim editor mode from `StatusJson.vim.mode`.
//! Port of upstream `VimMode.ts`.
//!
//! Claude Code populates `vim.mode` only when `editorMode: "vim"` is set in
//! the user's Claude Code settings (see
//! https://code.claude.com/docs/en/interactive-mode#vim-editor-mode).
//! Documented values: `NORMAL`, `INSERT`, `VISUAL`, `VISUAL LINE`.
//!
//! Rendering: `-- MODE --` in labelled form (mirrors vim's status-line
//! convention), bare `MODE` string in raw mode. Empty / absent field →
//! widget hides itself, so users who don't use vim mode see nothing.
//!
//! Users who render this widget typically also set `hideVimModeIndicator:
//! true` in `~/.claude/settings.json` to suppress Claude Code's own
//! below-prompt indicator (see
//! https://code.claude.com/docs/en/settings#statusline).

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::{is_raw, styled};

pub fn factory() -> Box<dyn Widget> {
    Box::new(VimMode)
}

pub struct VimMode;

impl Widget for VimMode {
    fn id(&self) -> &'static str {
        "vim-mode"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("brightBlack")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let mode = ctx
            .data
            .as_ref()
            .and_then(|d| d.vim.as_ref())
            .and_then(|v| v.mode.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(mode) = mode else {
            return Vec::new();
        };
        let text = if is_raw(spec) {
            mode.to_string()
        } else {
            format!("-- {mode} --")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{StatusJson, Vim};

    fn ctx_with_mode(mode: Option<&str>) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                vim: mode.map(|m| Vim {
                    mode: Some(m.to_string()),
                }),
                ..StatusJson::default()
            }),
            ..RenderContext::default()
        }
    }

    #[test]
    fn labelled_mode() {
        let ctx = ctx_with_mode(Some("INSERT"));
        let spans = VimMode.render(&WidgetSpec::new("1", "vim-mode"), &ctx);
        assert_eq!(spans[0].text, "-- INSERT --");
    }

    #[test]
    fn visual_line_mode_preserves_space() {
        let ctx = ctx_with_mode(Some("VISUAL LINE"));
        let spans = VimMode.render(&WidgetSpec::new("1", "vim-mode"), &ctx);
        assert_eq!(spans[0].text, "-- VISUAL LINE --");
    }

    #[test]
    fn raw_drops_dashes() {
        let mut spec = WidgetSpec::new("1", "vim-mode");
        spec.raw_value = Some(true);
        let ctx = ctx_with_mode(Some("NORMAL"));
        let spans = VimMode.render(&spec, &ctx);
        assert_eq!(spans[0].text, "NORMAL");
    }

    #[test]
    fn hides_when_vim_field_absent() {
        // No `vim` object at all — Claude Code omits it when editorMode
        // isn't set to "vim".
        let ctx = ctx_with_mode(None);
        let spans = VimMode.render(&WidgetSpec::new("1", "vim-mode"), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn hides_when_mode_field_missing() {
        // `vim` present but `mode` is None (shouldn't happen per spec but
        // guard against it).
        let ctx = RenderContext {
            data: Some(StatusJson {
                vim: Some(Vim { mode: None }),
                ..StatusJson::default()
            }),
            ..RenderContext::default()
        };
        let spans = VimMode.render(&WidgetSpec::new("1", "vim-mode"), &ctx);
        assert!(spans.is_empty());
    }

    #[test]
    fn hides_when_mode_string_empty_or_whitespace() {
        for empty in ["", "   ", "\t\n"] {
            let ctx = ctx_with_mode(Some(empty));
            let spans = VimMode.render(&WidgetSpec::new("1", "vim-mode"), &ctx);
            assert!(spans.is_empty(), "expected empty for mode={empty:?}");
        }
    }

    #[test]
    fn hides_with_no_status_json() {
        let spans = VimMode.render(
            &WidgetSpec::new("1", "vim-mode"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }
}
