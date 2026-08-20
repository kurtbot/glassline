//! Compile-time widget registry via [`phf`].
//!
//! Every built-in widget is a zero-arg factory that returns a boxed
//! [`Widget`](glassline_core::widget::Widget). External `ext:` widgets are
//! resolved elsewhere (P2 `glassline-ext` crate); they never appear here.

use glassline_core::widget::Widget;

use crate::builtins::{
    block_reset_timer, block_timer, claude_session_id, compaction_counter, context_bar,
    context_length, context_percentage, custom_text, cwd, git_branch, git_changes, git_root_dir,
    git_sha, git_status, link, model, output_style, separator, session_clock, session_cost,
    session_name, speed, thinking_effort, tokens_input, tokens_output, usage, version,
};

/// A zero-arg factory. Widgets are cheap to construct — no shared state.
pub type WidgetFactory = fn() -> Box<dyn Widget>;

/// The whole built-in widget catalogue, keyed on the `type` string that
/// appears in `settings.json`.
pub static WIDGETS: phf::Map<&'static str, WidgetFactory> = phf::phf_map! {
    "block-reset-timer" => block_reset_timer::factory,
    "block-timer" => block_timer::factory,
    "claude-session-id" => claude_session_id::factory,
    "compaction-counter" => compaction_counter::factory,
    "context-bar" => context_bar::factory,
    "context-length" => context_length::factory,
    "context-percentage" => context_percentage::factory,
    "current-working-dir" => cwd::factory,
    "custom-text" => custom_text::factory,
    "git-branch" => git_branch::factory,
    "git-changes" => git_changes::factory,
    "git-root-dir" => git_root_dir::factory,
    "git-sha" => git_sha::factory,
    "git-status" => git_status::factory,
    "input-speed" => speed::input_factory,
    "link" => link::factory,
    "model" => model::factory,
    "output-speed" => speed::output_factory,
    "output-style" => output_style::factory,
    "separator" => separator::factory,
    "session-clock" => session_clock::factory,
    "session-cost" => session_cost::factory,
    "session-name" => session_name::factory,
    "session-usage" => usage::session_usage_factory,
    "thinking-effort" => thinking_effort::factory,
    "tokens-input" => tokens_input::factory,
    "tokens-output" => tokens_output::factory,
    "total-speed" => speed::total_factory,
    "version" => version::factory,
    "weekly-opus-usage" => usage::weekly_opus_usage_factory,
    "weekly-reset-timer" => usage::weekly_reset_timer_factory,
    "weekly-sonnet-usage" => usage::weekly_sonnet_usage_factory,
    "weekly-usage" => usage::weekly_usage_factory,
};

/// Look up a built-in widget by ID.
#[must_use]
pub fn resolve(id: &str) -> Option<Box<dyn Widget>> {
    WIDGETS.get(id).map(|f| f())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_all_registered() {
        for (id, _) in WIDGETS.entries() {
            let widget = resolve(id).unwrap_or_else(|| panic!("registry entry {id} unresolved"));
            assert_eq!(widget.id(), *id, "widget id doesn't match registry key");
        }
    }

    #[test]
    fn resolve_unknown_widget_returns_none() {
        assert!(resolve("this-does-not-exist").is_none());
    }

    #[test]
    fn resolve_ext_prefix_returns_none() {
        // ext:* widgets never live in the built-in map — the loader routes
        // them into glassline-ext instead.
        assert!(resolve("ext:whatever").is_none());
    }
}
