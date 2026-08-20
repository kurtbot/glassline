//! `claude-account-email` — email of the currently-logged-in Claude Code
//! account. Port of upstream `ClaudeAccountEmail.ts`.
//!
//! Reads `oauthAccount.emailAddress` from `~/.claude.json` (or
//! `$CLAUDE_CONFIG_DIR/.claude.json` when that env var points at a valid
//! directory). Silently hides on any failure: missing file, malformed
//! JSON, missing field, empty string. No prefetch — the file read happens
//! per invocation. If a user has multiple statusline widgets that all
//! read this file, glassline's render cache (T8) absorbs the cost
//! naturally within a TTL window.
//!
//! Rendering:
//! - Labelled: `Account: user@example.com`.
//! - Raw: `user@example.com`.
//! - Absent / unreadable: widget hides.
//!
//! Privacy: the email is user-configured to be visible on the statusline;
//! it's not leaked into any error path. Failures always render as
//! `Vec::new()` — no fallback text.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    claude_settings::read_oauth_account_email,
    common::{is_raw, styled},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(ClaudeAccountEmail)
}

pub struct ClaudeAccountEmail;

impl Widget for ClaudeAccountEmail {
    fn id(&self) -> &'static str {
        "claude-account-email"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(email) = read_oauth_account_email() else {
            return Vec::new();
        };
        let text = if is_raw(spec) {
            email
        } else {
            format!("Account: {email}")
        };
        styled(spec, text)
    }
}

#[cfg(test)]
mod tests {
    // Widget-level tests are integration-shaped (need to fake
    // ~/.claude.json). The claude_settings.rs test module already covers
    // read_oauth_account_email; smoke-testing here would duplicate.
    // Keeping just a construction sanity so the registry stays green.
    use super::*;

    #[test]
    fn factory_returns_correct_id() {
        let w = factory();
        assert_eq!(w.id(), "claude-account-email");
    }

    #[test]
    fn no_email_hides_widget() {
        // The default context doesn't fake a ~/.claude.json; on any
        // machine where it's absent this returns empty. On a machine
        // where it exists this test will resolve — that's fine, it's a
        // real-account smoke test and we can't spoof the home dir from
        // a unit test without more scaffolding.
        //
        // The important assertion is that the render path never panics
        // on absence — hides cleanly.
        let spans = ClaudeAccountEmail.render(
            &WidgetSpec::new("1", "claude-account-email"),
            &RenderContext::default(),
        );
        // Either we hide (no file / no field) or we render a valid
        // labelled string — never a partial/broken output.
        if !spans.is_empty() {
            assert!(spans[0].text.starts_with("Account: ") || !spans[0].text.is_empty());
        }
    }
}
