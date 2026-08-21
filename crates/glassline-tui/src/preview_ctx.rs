//! Canned [`RenderContext`] for the editor's live-preview strip.
//!
//! Every field is seeded so preview surfaces a realistic-looking line
//! regardless of which widgets the current scratch settings enable.
//! Values are frozen constants — the preview must be deterministic
//! across environments so screens don't drift with the developer's
//! real repo/session.

use glassline_core::{
    render_context::{
        BlockMetrics, CacheTimerState, CompactionData, CompactionTriggers, GitData, RenderContext,
        RenderUsageData, SkillsMetrics, SpeedMetrics, TokenMetrics,
    },
    status_json::{
        ContextWindow, Cost, CurrentUsage, Effort, ModelInfo, OutputStyle, StatusJson, Vim,
        Workspace, WorkspaceRepo,
    },
};

/// Build a plausible `RenderContext` for the preview strip.
///
/// Frozen constants only — no real filesystem or git access. Values
/// are chosen so every category of widget renders something visible:
/// tokens are non-zero, context is 34 % (mid-band on any threshold
/// palette), usage lines have values, git has a branch + diff counts,
/// `workspace.repo` is populated so the origin/host widgets exercise
/// their fast path.
#[must_use]
pub fn canned_context() -> RenderContext {
    RenderContext {
        data: Some(canned_status_json()),
        token_metrics: Some(TokenMetrics {
            input: 42_000,
            output: 8_500,
            cache_read: 12_000,
            cache_creation: 3_000,
            context_length: 68_000,
        }),
        speed_metrics: Some(SpeedMetrics {
            total_duration_ms: 60_000,
            input_tokens: 51_000,
            output_tokens: 8_500,
            request_count: 12,
        }),
        windowed_speed_metrics: None,
        usage_data: Some(canned_usage()),
        session_duration: Some("42m".into()),
        block_metrics: Some(BlockMetrics {
            block_id: Some("preview-block".into()),
            started_at: Some("2026-08-20T12:00:00Z".into()),
            resets_at: Some("2026-08-20T17:00:00Z".into()),
        }),
        skills_metrics: Some(SkillsMetrics::default()),
        compaction_data: Some(CompactionData {
            count: 2,
            by_trigger: CompactionTriggers {
                auto: 1,
                manual: 1,
                unknown: 0,
            },
            tokens_reclaimed: 12_000,
        }),
        cache_timer: Some(CacheTimerState {
            working: false,
            last_touch_ms: Some(1_755_000_000_000),
        }),
        last_effort_level: Some("high".into()),
        terminal_width: Some(120),
        is_preview: true,
        minimalist: false,
        git_cache_ttl_seconds: Some(5),
        git_review_needs_checks: false,
        line_index: 0,
        global_separator_index: 0,
        global_powerline_theme_index: 0,
        global_powerline_start_cap_index: 0,
        git_data: Some(GitData {
            changed_files: Some(4),
            insertions: Some(87),
            deletions: Some(23),
        }),
        now_ms: 1_755_000_000_000,
    }
}

fn canned_status_json() -> StatusJson {
    StatusJson {
        session_id: Some("preview-session".into()),
        session_name: Some("preview".into()),
        transcript_path: None,
        cwd: Some("/home/u/proj".into()),
        model: Some(ModelInfo::Full {
            id: Some("claude-sonnet-5".into()),
            display_name: Some("Sonnet 5".into()),
        }),
        workspace: Some(Workspace {
            current_dir: Some("/home/u/proj".into()),
            project_dir: Some("/home/u/proj".into()),
            repo: Some(WorkspaceRepo {
                host: Some("github.com".into()),
                owner: Some("kurtbot".into()),
                name: Some("glassline".into()),
            }),
        }),
        version: Some("2.1.90".into()),
        output_style: Some(OutputStyle {
            name: Some("default".into()),
        }),
        effort: Some(Effort {
            level: Some("high".into()),
        }),
        cost: Some(Cost {
            total_cost_usd: Some(0.42),
            total_duration_ms: Some(2_450_000.0),
            total_api_duration_ms: Some(51_000.0),
            total_lines_added: Some(156.0),
            total_lines_removed: Some(23.0),
        }),
        context_window: Some(ContextWindow {
            context_window_size: Some(200_000.0),
            total_input_tokens: Some(57_000.0),
            total_output_tokens: Some(8_500.0),
            current_usage: Some(CurrentUsage::Breakdown {
                input_tokens: Some(42_000.0),
                output_tokens: Some(8_500.0),
                cache_creation_input_tokens: Some(3_000.0),
                cache_read_input_tokens: Some(12_000.0),
            }),
            used_percentage: Some(34.0),
            remaining_percentage: Some(66.0),
            usable_percentage: Some(58.0),
        }),
        vim: Some(Vim {
            mode: Some("NORMAL".into()),
        }),
        worktree: None,
        rate_limits: None,
        hook_event_name: None,
        extras: std::collections::BTreeMap::new(),
    }
}

fn canned_usage() -> RenderUsageData {
    RenderUsageData {
        session_usage: Some(42.0),
        session_reset_at: Some("2026-08-20T18:00:00Z".into()),
        weekly_usage: Some(61.0),
        weekly_reset_at: Some("2026-08-27T00:00:00Z".into()),
        weekly_sonnet_usage: Some(12.0),
        weekly_sonnet_reset_at: Some("2026-08-27T00:00:00Z".into()),
        weekly_opus_usage: Some(38.0),
        weekly_opus_reset_at: Some("2026-08-27T00:00:00Z".into()),
        fable_usage: None,
        fable_reset_at: None,
        extra_usage_enabled: Some(false),
        extra_usage_limit: None,
        extra_usage_used: None,
        extra_usage_utilization: None,
        extra_usage_currency: Some("USD".into()),
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::settings::Settings;
    use glassline_render::render_to_string;

    #[test]
    fn canned_context_is_populated() {
        let ctx = canned_context();
        assert!(ctx.data.is_some());
        assert!(ctx.token_metrics.is_some());
        assert!(ctx.usage_data.is_some());
        assert!(ctx.git_data.is_some());
    }

    #[test]
    fn canned_workspace_repo_is_populated() {
        // The origin fast path in git-origin-{host,owner,repo,owner-repo}
        // reads workspace.repo. Preview must populate it or those
        // widgets fall back to shell-out (and would fail on the
        // developer's machine when it lacks git).
        let ctx = canned_context();
        let repo = ctx
            .data
            .expect("status_json")
            .workspace
            .expect("workspace")
            .repo
            .expect("workspace.repo");
        assert_eq!(repo.host.as_deref(), Some("github.com"));
        assert_eq!(repo.owner.as_deref(), Some("kurtbot"));
        assert_eq!(repo.name.as_deref(), Some("glassline"));
    }

    #[test]
    fn pipeline_does_not_blow_up_on_canned_context() {
        // Sanity: the preview pipeline must not error on the canned
        // context + an empty Settings. Non-empty output is fine —
        // Settings::default() may still emit a blank line.
        let ctx = canned_context();
        let s = Settings::default();
        let _out = render_to_string(ctx, &s).expect("pipeline must not fail on empty settings");
    }
}
