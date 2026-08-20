//! Compile-time widget registry via [`phf`].
//!
//! Every built-in widget is a zero-arg factory that returns a boxed
//! [`Widget`](glassline_core::widget::Widget). External `ext:` widgets are
//! resolved elsewhere (P2 `glassline-ext` crate); they never appear here.

use glassline_core::widget::Widget;

use crate::builtins::{
    block_reset_timer, block_timer, cache_hit_rate, cache_read, cache_write, claude_session_id,
    compaction_counter, context_bar, context_length, context_percentage, context_percentage_usable,
    context_window, custom_command, custom_symbol, custom_text, cwd, extra_usage_remaining,
    extra_usage_used, extra_usage_utilization, fable_weekly_usage, free_memory, git_ahead_behind,
    git_branch, git_changes, git_ci_status, git_clean_status, git_conflicts, git_deletions,
    git_insertions, git_is_fork, git_origin_owner, git_origin_owner_repo, git_origin_repo, git_pr,
    git_root_dir, git_sha, git_staged, git_staged_files, git_status, git_unstaged,
    git_unstaged_files, git_untracked, git_untracked_files, git_upstream_owner,
    git_upstream_owner_repo, git_upstream_repo, git_worktree, git_worktree_branch,
    git_worktree_mode, git_worktree_name, git_worktree_original_branch, jj_bookmarks, jj_changes,
    jj_deletions, jj_description, jj_insertions, jj_revision, jj_root_dir, jj_workspace, link,
    model, output_style, separator, session_clock, session_cost, session_name, skills, speed,
    terminal_width, thinking_effort, tokens_cached, tokens_input, tokens_output, tokens_total,
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
    "custom-command" => custom_command::factory,
    "custom-symbol" => custom_symbol::factory,
    "custom-text" => custom_text::factory,
    "extra-usage-remaining" => extra_usage_remaining::factory,
    "extra-usage-used" => extra_usage_used::factory,
    "extra-usage-utilization" => extra_usage_utilization::factory,
    "fable-weekly-usage" => fable_weekly_usage::factory,
    "free-memory" => free_memory::factory,
    "git-ahead-behind" => git_ahead_behind::factory,
    "git-branch" => git_branch::factory,
    "git-changes" => git_changes::factory,
    "git-ci-status" => git_ci_status::factory,
    "git-clean-status" => git_clean_status::factory,
    "git-conflicts" => git_conflicts::factory,
    "git-deletions" => git_deletions::factory,
    "git-insertions" => git_insertions::factory,
    "git-is-fork" => git_is_fork::factory,
    "git-origin-owner" => git_origin_owner::factory,
    "git-origin-owner-repo" => git_origin_owner_repo::factory,
    "git-origin-repo" => git_origin_repo::factory,
    "git-pr" => git_pr::factory,
    "git-root-dir" => git_root_dir::factory,
    "git-sha" => git_sha::factory,
    "git-staged" => git_staged::factory,
    "git-staged-files" => git_staged_files::factory,
    "git-status" => git_status::factory,
    "git-unstaged" => git_unstaged::factory,
    "git-unstaged-files" => git_unstaged_files::factory,
    "git-untracked" => git_untracked::factory,
    "git-untracked-files" => git_untracked_files::factory,
    "git-upstream-owner" => git_upstream_owner::factory,
    "git-upstream-owner-repo" => git_upstream_owner_repo::factory,
    "git-upstream-repo" => git_upstream_repo::factory,
    "git-worktree" => git_worktree::factory,
    "git-worktree-branch" => git_worktree_branch::factory,
    "git-worktree-mode" => git_worktree_mode::factory,
    "git-worktree-name" => git_worktree_name::factory,
    "git-worktree-original-branch" => git_worktree_original_branch::factory,
    "input-speed" => speed::input_factory,
    "jj-bookmarks" => jj_bookmarks::factory,
    "jj-changes" => jj_changes::factory,
    "jj-deletions" => jj_deletions::factory,
    "jj-description" => jj_description::factory,
    "jj-insertions" => jj_insertions::factory,
    "jj-revision" => jj_revision::factory,
    "jj-root-dir" => jj_root_dir::factory,
    "jj-workspace" => jj_workspace::factory,
    "link" => link::factory,
    "model" => model::factory,
    "output-speed" => speed::output_factory,
    "output-style" => output_style::factory,
    "separator" => separator::factory,
    "session-clock" => session_clock::factory,
    "session-cost" => session_cost::factory,
    "session-name" => session_name::factory,
    "session-usage" => usage::session_usage_factory,
    "skills" => skills::factory,
    "terminal-width" => terminal_width::factory,
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
