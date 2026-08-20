//! Compile-time widget registry via [`phf`].
//!
//! Every built-in widget is a zero-arg factory that returns a boxed
//! [`Widget`](glassline_core::widget::Widget). External `ext:` widgets are
//! resolved elsewhere (P2 `glassline-ext` crate); they never appear here.

use glassline_core::widget::Widget;

use crate::builtins::{
    block_reset_timer, block_timer, cache_hit_rate, cache_read, cache_write, claude_session_id,
    compaction_counter, context_bar, context_length, context_percentage, context_percentage_usable,
    context_window, custom_text, cwd, extra_usage_remaining, extra_usage_used,
    extra_usage_utilization, fable_weekly_usage, git_branch, git_changes, git_clean_status,
    git_conflicts, git_deletions, git_insertions, git_root_dir, git_sha, git_staged,
    git_staged_files, git_status, git_unstaged, git_unstaged_files, git_untracked,
    git_untracked_files, link, model, output_style, separator, session_clock, session_cost,
    session_name, speed, thinking_effort, tokens_cached, tokens_input, tokens_output, tokens_total,
    usage, version,
};

/// A zero-arg factory. Widgets are cheap to construct — no shared state.
pub type WidgetFactory = fn() -> Box<dyn Widget>;

/// The whole built-in widget catalogue, keyed on the `type` string that
/// appears in `settings.json`.
pub static WIDGETS: phf::Map<&'static str, WidgetFactory> = phf::phf_map! {
    "block-reset-timer" => block_reset_timer::factory,
    "block-timer" => block_timer::factory,
    "cache-hit-rate" => cache_hit_rate::factory,
    "cache-read" => cache_read::factory,
    "cache-write" => cache_write::factory,
    "claude-session-id" => claude_session_id::factory,
    "compaction-counter" => compaction_counter::factory,
    "context-bar" => context_bar::factory,
    "context-length" => context_length::factory,
    "context-percentage" => context_percentage::factory,
    "context-percentage-usable" => context_percentage_usable::factory,
    "context-window" => context_window::factory,
    "current-working-dir" => cwd::factory,
    "custom-text" => custom_text::factory,
    "extra-usage-remaining" => extra_usage_remaining::factory,
    "extra-usage-used" => extra_usage_used::factory,
    "extra-usage-utilization" => extra_usage_utilization::factory,
    "fable-weekly-usage" => fable_weekly_usage::factory,
    "git-branch" => git_branch::factory,
    "git-changes" => git_changes::factory,
    "git-clean-status" => git_clean_status::factory,
    "git-conflicts" => git_conflicts::factory,
    "git-deletions" => git_deletions::factory,
    "git-insertions" => git_insertions::factory,
    "git-root-dir" => git_root_dir::factory,
    "git-sha" => git_sha::factory,
    "git-staged" => git_staged::factory,
    "git-staged-files" => git_staged_files::factory,
    "git-status" => git_status::factory,
    "git-unstaged" => git_unstaged::factory,
    "git-unstaged-files" => git_unstaged_files::factory,
    "git-untracked" => git_untracked::factory,
    "git-untracked-files" => git_untracked_files::factory,
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
    "tokens-cached" => tokens_cached::factory,
    "tokens-input" => tokens_input::factory,
    "tokens-output" => tokens_output::factory,
    "tokens-total" => tokens_total::factory,
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
