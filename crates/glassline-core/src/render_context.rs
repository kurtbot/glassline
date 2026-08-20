//! [`RenderContext`] — everything a widget needs to render itself.
//!
//! Mirrors [`src/types/RenderContext.ts`](https://github.com/sirmalloc/ccstatusline).
//! The heavier substructures (`TokenMetrics`, `SpeedMetrics`, `SkillsMetrics`,
//! …) live behind placeholder types for now — P1 will flesh them out as the
//! transcript parser lands.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::status_json::StatusJson;

/// All runtime data a widget can read.
///
/// Field names deliberately match the TS shape so the port stays 1:1 and the
/// review can grep for `context.foo` across both sources.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    pub data: Option<StatusJson>,
    pub token_metrics: Option<TokenMetrics>,
    pub speed_metrics: Option<SpeedMetrics>,
    pub windowed_speed_metrics: Option<BTreeMap<String, SpeedMetrics>>,
    pub usage_data: Option<RenderUsageData>,
    pub session_duration: Option<String>,
    pub block_metrics: Option<BlockMetrics>,
    pub skills_metrics: Option<SkillsMetrics>,
    pub compaction_data: Option<CompactionData>,
    pub terminal_width: Option<usize>,
    pub is_preview: bool,
    pub minimalist: bool,
    pub git_cache_ttl_seconds: Option<u32>,
    pub git_review_needs_checks: bool,
    pub line_index: usize,
    pub global_separator_index: usize,
    pub global_powerline_theme_index: usize,
    pub global_powerline_start_cap_index: usize,
    pub git_data: Option<GitData>,
    /// Wall-clock time as unix-epoch milliseconds. Populated by the render
    /// binary at the top of `main()`; animations key on this value so
    /// successive refreshes produce successive frames. `0` in the default
    /// context, which effectively pins to unix epoch — fine for unit tests.
    pub now_ms: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct TokenMetrics {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
    /// `input + cache_read + cache_creation` for the most recent
    /// main-chain turn, corrected for compaction (see design §4.6).
    pub context_length: u64,
}

impl TokenMetrics {
    /// `cache_read + cache_creation`.
    #[must_use]
    pub fn cached(&self) -> u64 {
        self.cache_read + self.cache_creation
    }

    /// `input + output + cached()`.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cached()
    }
}

/// Speed metrics aggregate. Widgets derive tokens/sec on demand from
/// `input_tokens / (total_duration_ms / 1000)` etc, so the raw totals live
/// here and the tokens/sec computation is a widget-side helper.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SpeedMetrics {
    pub total_duration_ms: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub request_count: u64,
}

impl SpeedMetrics {
    /// input_tokens + output_tokens.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }

    /// input_tokens per second. `None` when duration is zero.
    #[must_use]
    pub fn input_per_sec(&self) -> Option<f64> {
        self.per_sec(self.input_tokens)
    }

    /// output_tokens per second. `None` when duration is zero.
    #[must_use]
    pub fn output_per_sec(&self) -> Option<f64> {
        self.per_sec(self.output_tokens)
    }

    /// total_tokens per second. `None` when duration is zero.
    #[must_use]
    pub fn total_per_sec(&self) -> Option<f64> {
        self.per_sec(self.total_tokens())
    }

    fn per_sec(&self, tokens: u64) -> Option<f64> {
        if self.total_duration_ms == 0 {
            None
        } else {
            Some(tokens as f64 / (self.total_duration_ms as f64 / 1000.0))
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct BlockMetrics {
    pub block_id: Option<String>,
    pub started_at: Option<String>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct SkillsMetrics {
    pub session_id: Option<String>,
    pub skills_invoked: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CompactionData {
    pub count: u64,
    pub by_trigger: CompactionTriggers,
    pub tokens_reclaimed: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct CompactionTriggers {
    pub auto: u64,
    pub manual: u64,
    pub unknown: u64,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct RenderUsageData {
    pub session_usage: Option<f64>,
    pub session_reset_at: Option<String>,
    pub weekly_usage: Option<f64>,
    pub weekly_reset_at: Option<String>,
    pub weekly_sonnet_usage: Option<f64>,
    pub weekly_sonnet_reset_at: Option<String>,
    pub weekly_opus_usage: Option<f64>,
    pub weekly_opus_reset_at: Option<String>,
    pub fable_usage: Option<f64>,
    pub fable_reset_at: Option<String>,
    pub extra_usage_enabled: Option<bool>,
    pub extra_usage_limit: Option<f64>,
    pub extra_usage_used: Option<f64>,
    pub extra_usage_utilization: Option<f64>,
    pub extra_usage_currency: Option<String>,
    pub error: Option<UsageError>,
}

/// The set of ways usage-data fetching can fail (mirrors the TS union).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UsageError {
    NoCredentials,
    Timeout,
    RateLimited,
    ApiError,
    ParseError,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct GitData {
    pub changed_files: Option<u64>,
    pub insertions: Option<u64>,
    pub deletions: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_default_is_empty() {
        let ctx = RenderContext::default();
        assert!(ctx.data.is_none());
        assert!(!ctx.is_preview);
        assert_eq!(ctx.line_index, 0);
    }

    #[test]
    fn usage_error_kebab_case() {
        assert_eq!(
            serde_json::to_string(&UsageError::NoCredentials).unwrap(),
            "\"no-credentials\""
        );
        let parsed: UsageError = serde_json::from_str("\"rate-limited\"").unwrap();
        assert_eq!(parsed, UsageError::RateLimited);
    }
}
