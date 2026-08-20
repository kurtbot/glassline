//! Streaming scan of Claude Code's `transcript.jsonl` for token / compaction /
//! duration metrics.
//!
//! Ports the core loop of `src/utils/jsonl-metrics.ts` +
//! `src/utils/compaction.ts` from ccstatusline. Notable divergences from
//! the TS impl for the P1 slice:
//!   - **Speed metrics** (`getSpeedMetrics`, windowed variants) are DEFERRED
//!     to a later T-1.5b pass — the speed math is non-trivial and none of
//!     the P1 MVP widgets depend on it yet.
//!   - **Sidechain (subagent) filtering** is applied to timestamp tracking
//!     but NOT to the raw token sums, matching the TS behaviour:
//!     `getTokenMetrics` sums usage across every turn, then picks the most
//!     recent *main-chain* turn for `context_length`.
//!
//! The scan is line-based rather than a `serde_json::StreamDeserializer`:
//! transcripts occasionally contain a truncated final line (write in
//! progress), which the streaming deserialiser refuses to skip past. The
//! line loop treats every unparseable line as `null` (matches TS's
//! `parseJsonlLine` returning null on error) so a broken line doesn't lose
//! the metrics from every earlier line.

use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

use glassline_core::{
    render_context::{CompactionData, CompactionTriggers, SpeedMetrics, TokenMetrics},
    widget::WidgetRequirements,
};
use serde::Deserialize;
use thiserror::Error;
use time::OffsetDateTime;

/// Everything one scan pass produces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranscriptScan {
    pub tokens: TokenMetrics,
    pub compaction: CompactionData,
    /// Formatted like the TS `getSessionDuration` output — `<1m`, `42m`,
    /// `3hr`, `1hr 12m`. `None` when we couldn't recover both endpoints.
    pub session_duration: Option<String>,
    pub speed: SpeedMetrics,
    /// `true` when the newest main-chain (non-sidechain) entry is a user
    /// row — Claude Code is mid-turn processing a prompt or tool result
    /// and the prompt cache is refreshing. Consumed by `cache-timer`.
    pub cache_working: bool,
    /// Unix-epoch milliseconds of the newest main-chain assistant entry
    /// that actually touched the prompt cache (nonzero
    /// `cache_read_input_tokens + cache_creation_input_tokens`, or missing
    /// usage data — older transcript formats without usage counters are
    /// assumed to have touched the cache). `None` when no such entry
    /// exists.
    pub cache_last_touch_ms: Option<u64>,
}

/// Whether an assistant transcript entry actually touched the prompt cache.
///
/// Upstream policy (`hasCacheActivity` in CacheTimer.ts): rows without
/// usage data cannot be classified and are assumed to be cache events so
/// older transcript formats keep driving the countdown. Rows with usage
/// data are cache events only when `cache_read + cache_creation > 0`.
fn has_cache_activity(event: &TranscriptEvent) -> bool {
    let Some(msg) = event.message.as_ref() else {
        return true;
    };
    let Some(usage) = msg.usage.as_ref() else {
        return true;
    };
    (usage.cache_read_input_tokens.unwrap_or(0)
        + usage.cache_creation_input_tokens.unwrap_or(0))
        > 0
}

/// Fatal I/O error. Missing file / empty file / all-unparseable lines are
/// NOT errors — they return `TranscriptScan::default()` (all zeros).
#[derive(Debug, Error)]
pub enum ScanError {
    #[error("could not open transcript {path}: {source}")]
    Open {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

/// Scan `path` under the union of every visible widget's data needs.
///
/// The `needs` bitset lets a caller with only a `compaction-counter` widget
/// skip the token-aggregation branch, or vice versa. In the P1 slice most
/// callers just pass `WidgetRequirements::TRANSCRIPT | ...COMPACTION` and
/// take the whole scan — that's still O(1) memory over the file size.
pub fn scan(path: &Path, needs: WidgetRequirements) -> Result<TranscriptScan, ScanError> {
    if !path.exists() {
        return Ok(TranscriptScan::default());
    }
    let file = File::open(path).map_err(|e| ScanError::Open {
        path: path.to_path_buf(),
        source: e,
    })?;
    let reader = BufReader::new(file);

    let want_tokens = needs.contains(WidgetRequirements::TRANSCRIPT);
    let want_compaction = needs.contains(WidgetRequirements::COMPACTION);
    let want_duration = needs.contains(WidgetRequirements::SESSION_CLOCK);
    let want_speed = needs.contains(WidgetRequirements::SPEED);
    // cache-timer state comes essentially free during the same iteration
    // (one bool + one Option<Timestamp>). Compute it whenever any
    // transcript work is happening; the widget itself gates on
    // TRANSCRIPT via its declared requirement.
    let want_cache_timer = want_tokens
        || want_compaction
        || want_duration
        || want_speed
        || needs.contains(WidgetRequirements::CACHE);

    let mut acc = ScanAccumulator::default();

    for (line_idx, line_result) in reader.lines().enumerate() {
        let Ok(line) = line_result else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<TranscriptEvent>(&line) else {
            continue;
        };

        if want_compaction && event.is_compact_boundary() {
            acc.record_compaction(&event);
        }
        if want_tokens && event.message.as_ref().is_some_and(|m| m.usage.is_some()) {
            acc.record_usage(&event, line_idx);
        }
        // Compaction boundaries impact token context tracking even when
        // the caller only asked for token metrics.
        if want_tokens && event.is_compact_boundary() {
            acc.compaction_boundary_line = Some(line_idx);
            acc.compaction_boundary_post_tokens = event.compact_post_tokens();
        }
        if want_duration && let Some(ts) = event.parsed_timestamp() {
            if acc.first_timestamp.is_none() {
                acc.first_timestamp = Some(ts);
            }
            acc.last_timestamp = Some(ts);
        }
        if want_speed && !event.is_api_error_message && !event.is_sidechain {
            acc.record_speed(&event);
        }
        if want_cache_timer && !event.is_sidechain {
            acc.record_cache_timer(&event);
        }
    }

    Ok(acc.finish(
        want_tokens,
        want_compaction,
        want_duration,
        want_speed,
        want_cache_timer,
    ))
}

// ---------- accumulator ----------

#[derive(Default)]
#[allow(dead_code)] // input/output/cache_* are held for future speed-widget wiring.
struct ScanAccumulator {
    // token totals
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    // stop_reason gating (TS `hasStopReasonField`)
    has_stop_reason_field: bool,
    // per-line collected usage entries for the two-pass stop_reason filter
    usage_entries: Vec<UsageEntry>,
    // compaction stats
    compaction_count: u64,
    compaction_auto: u64,
    compaction_manual: u64,
    compaction_unknown: u64,
    compaction_reclaimed: u64,
    compaction_boundary_line: Option<usize>,
    compaction_boundary_post_tokens: Option<u64>,
    // duration
    first_timestamp: Option<OffsetDateTime>,
    last_timestamp: Option<OffsetDateTime>,
    // speed
    last_user_ts: Option<OffsetDateTime>,
    speed_requests: Vec<SpeedRequestRec>,
    // cache-timer
    /// Whether the latest main-chain entry seen was a `user` row.
    /// `false` initially; set true on user rows, false on assistant rows.
    last_main_chain_was_user: bool,
    /// Newest main-chain assistant entry with cache activity.
    newest_cache_touch_ts: Option<OffsetDateTime>,
}

#[derive(Debug, Clone)]
struct SpeedRequestRec {
    input_tokens: u64,
    output_tokens: u64,
    interval: Option<(OffsetDateTime, OffsetDateTime)>,
}

#[derive(Debug, Clone)]
struct UsageEntry {
    line_idx: usize,
    input: u64,
    output: u64,
    cache_read: u64,
    cache_creation: u64,
    is_sidechain: bool,
    is_api_error: bool,
    timestamp: Option<OffsetDateTime>,
    stop_reason: StopReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    Absent,
    Null,
    Some,
}

impl ScanAccumulator {
    fn record_compaction(&mut self, event: &TranscriptEvent) {
        self.compaction_count += 1;
        match event.compact_trigger() {
            Some("auto") => self.compaction_auto += 1,
            Some("manual") => self.compaction_manual += 1,
            _ => self.compaction_unknown += 1,
        }
        if let (Some(pre), Some(post)) = (event.compact_pre_tokens(), event.compact_post_tokens()) {
            self.compaction_reclaimed += pre.saturating_sub(post);
        }
    }

    fn record_usage(&mut self, event: &TranscriptEvent, line_idx: usize) {
        let Some(msg) = event.message.as_ref() else {
            return;
        };
        let Some(usage) = msg.usage.as_ref() else {
            return;
        };
        let stop_reason = msg.stop_reason_presence();
        if stop_reason != StopReason::Absent {
            self.has_stop_reason_field = true;
        }
        self.usage_entries.push(UsageEntry {
            line_idx,
            input: usage.input_tokens.unwrap_or(0),
            output: usage.output_tokens.unwrap_or(0),
            cache_read: usage.cache_read_input_tokens.unwrap_or(0),
            cache_creation: usage.cache_creation_input_tokens.unwrap_or(0),
            is_sidechain: event.is_sidechain,
            is_api_error: event.is_api_error_message,
            timestamp: event.parsed_timestamp(),
            stop_reason,
        });
    }

    fn record_cache_timer(&mut self, event: &TranscriptEvent) {
        match event.r#type.as_deref() {
            Some("user") => {
                self.last_main_chain_was_user = true;
            }
            Some("assistant") => {
                self.last_main_chain_was_user = false;
                if event.is_api_error_message {
                    return;
                }
                if !has_cache_activity(event) {
                    return;
                }
                let Some(ts) = event.parsed_timestamp() else {
                    return;
                };
                if self
                    .newest_cache_touch_ts
                    .is_none_or(|prev| ts > prev)
                {
                    self.newest_cache_touch_ts = Some(ts);
                }
            }
            _ => {}
        }
    }

    fn record_speed(&mut self, event: &TranscriptEvent) {
        let Some(ts) = event.parsed_timestamp() else {
            return;
        };
        if event.r#type.as_deref() == Some("user") {
            self.last_user_ts = Some(ts);
            return;
        }
        if event.r#type.as_deref() == Some("assistant")
            && let Some(msg) = event.message.as_ref()
            && let Some(usage) = msg.usage.as_ref()
        {
            let interval = self
                .last_user_ts
                .and_then(|start| if ts > start { Some((start, ts)) } else { None });
            self.speed_requests.push(SpeedRequestRec {
                input_tokens: usage.input_tokens.unwrap_or(0),
                output_tokens: usage.output_tokens.unwrap_or(0),
                interval,
            });
        }
    }

    fn finish(
        mut self,
        want_tokens: bool,
        want_compaction: bool,
        want_duration: bool,
        want_speed: bool,
        want_cache_timer: bool,
    ) -> TranscriptScan {
        let mut scan = TranscriptScan::default();

        if want_tokens {
            self.finish_tokens(&mut scan);
        }
        if want_compaction {
            scan.compaction = CompactionData {
                count: self.compaction_count,
                by_trigger: CompactionTriggers {
                    auto: self.compaction_auto,
                    manual: self.compaction_manual,
                    unknown: self.compaction_unknown,
                },
                tokens_reclaimed: self.compaction_reclaimed,
            };
        }
        if want_duration {
            scan.session_duration = format_duration(self.first_timestamp, self.last_timestamp);
        }
        if want_speed {
            scan.speed = self.finish_speed();
        }
        if want_cache_timer {
            scan.cache_working = self.last_main_chain_was_user;
            scan.cache_last_touch_ms = self.newest_cache_touch_ts.map(|ts| {
                let millis = ts.unix_timestamp_nanos() / 1_000_000;
                u64::try_from(millis).unwrap_or(0)
            });
        }
        scan
    }

    fn finish_speed(&mut self) -> SpeedMetrics {
        let mut input_tokens = 0u64;
        let mut output_tokens = 0u64;
        let mut intervals: Vec<(i128, i128)> = Vec::new();
        for req in &self.speed_requests {
            input_tokens += req.input_tokens;
            output_tokens += req.output_tokens;
            if let Some((s, e)) = req.interval {
                intervals.push((
                    (s.unix_timestamp_nanos() / 1_000_000),
                    (e.unix_timestamp_nanos() / 1_000_000),
                ));
            }
        }
        let total_duration_ms = merge_intervals_duration_ms(intervals);
        SpeedMetrics {
            total_duration_ms,
            input_tokens,
            output_tokens,
            request_count: self.speed_requests.len() as u64,
        }
    }

    fn finish_tokens(&mut self, scan: &mut TranscriptScan) {
        // Two-pass: if any usage entry had `stop_reason` present in the
        // schema, only count entries whose stop_reason is a Some (finalised)
        // OR the very last one whose stop_reason is Null (in-progress
        // streaming turn). Matches TS `entriesToCount`.
        let entries: Vec<UsageEntry> = if self.has_stop_reason_field {
            let last_null_idx = self
                .usage_entries
                .iter()
                .rposition(|e| e.stop_reason == StopReason::Null);
            self.usage_entries
                .iter()
                .enumerate()
                .filter(|(idx, e)| e.stop_reason == StopReason::Some || Some(*idx) == last_null_idx)
                .map(|(_, e)| e.clone())
                .collect()
        } else {
            std::mem::take(&mut self.usage_entries)
        };

        let mut input = 0u64;
        let mut output = 0u64;
        let mut cache_read = 0u64;
        let mut cache_creation = 0u64;

        let mut most_recent_main: Option<&UsageEntry> = None;
        let mut most_recent_main_ts: Option<OffsetDateTime> = None;
        let mut most_recent_post_compact: Option<&UsageEntry> = None;
        let mut most_recent_post_compact_ts: Option<OffsetDateTime> = None;

        for entry in &entries {
            input += entry.input;
            output += entry.output;
            cache_read += entry.cache_read;
            cache_creation += entry.cache_creation;

            if entry.is_sidechain || entry.is_api_error {
                continue;
            }
            let Some(ts) = entry.timestamp else {
                continue;
            };
            if most_recent_main_ts.is_none_or(|prev| ts > prev) {
                most_recent_main_ts = Some(ts);
                most_recent_main = Some(entry);
            }
            if let Some(boundary_line) = self.compaction_boundary_line
                && entry.line_idx > boundary_line
                && most_recent_post_compact_ts.is_none_or(|prev| ts > prev)
            {
                most_recent_post_compact_ts = Some(ts);
                most_recent_post_compact = Some(entry);
            }
        }

        let context_length = if self.compaction_boundary_line.is_some() {
            most_recent_post_compact
                .map(|e| e.input + e.cache_read + e.cache_creation)
                .or(self.compaction_boundary_post_tokens)
                .unwrap_or(0)
        } else {
            most_recent_main
                .map(|e| e.input + e.cache_read + e.cache_creation)
                .unwrap_or(0)
        };

        scan.tokens = TokenMetrics {
            input,
            output,
            cache_read,
            cache_creation,
            context_length,
        };
    }
}

// ---------- event schema ----------

/// Only the fields we currently consume live here; everything else is
/// silently dropped by serde's default behaviour (no `deny_unknown_fields`).
#[derive(Debug, Clone, Deserialize, Default)]
struct TranscriptEvent {
    #[serde(default)]
    r#type: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default, rename = "isSidechain")]
    is_sidechain: bool,
    #[serde(default, rename = "isApiErrorMessage")]
    is_api_error_message: bool,
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default)]
    message: Option<TranscriptMessage>,
    #[serde(default, rename = "compactMetadata")]
    compact_metadata: Option<CompactMetadata>,
}

impl TranscriptEvent {
    fn is_compact_boundary(&self) -> bool {
        self.r#type.as_deref() == Some("system")
            && self.subtype.as_deref() == Some("compact_boundary")
            && !self.is_sidechain
    }

    fn compact_trigger(&self) -> Option<&str> {
        self.compact_metadata.as_ref()?.trigger.as_deref()
    }

    fn compact_pre_tokens(&self) -> Option<u64> {
        self.compact_metadata.as_ref()?.pre_tokens.map(u64_from_f64)
    }

    fn compact_post_tokens(&self) -> Option<u64> {
        self.compact_metadata
            .as_ref()?
            .post_tokens
            .map(u64_from_f64)
    }

    fn parsed_timestamp(&self) -> Option<OffsetDateTime> {
        parse_iso_timestamp(self.timestamp.as_deref()?)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TranscriptMessage {
    #[serde(default)]
    usage: Option<TranscriptUsage>,
    /// Everything else on the message object. We need this to distinguish
    /// `"stop_reason": null` (streaming turn in progress) from an absent
    /// key (transcript format has no `stop_reason` at all) — serde
    /// `Option<T>` collapses null and absent to the same `None`.
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

impl TranscriptMessage {
    fn stop_reason_presence(&self) -> StopReason {
        match self.other.get("stop_reason") {
            None => StopReason::Absent,
            Some(serde_json::Value::Null) => StopReason::Null,
            Some(_) => StopReason::Some,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
struct TranscriptUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct CompactMetadata {
    #[serde(default)]
    trigger: Option<String>,
    #[serde(default, rename = "preTokens")]
    pre_tokens: Option<f64>,
    #[serde(default, rename = "postTokens")]
    post_tokens: Option<f64>,
}

// ---------- helpers ----------

/// Merge overlapping `[start_ms, end_ms]` intervals and return the total
/// covered duration in ms. Port of TS `mergeIntervals` + `getIntervalsDurationMs`.
fn merge_intervals_duration_ms(mut intervals: Vec<(i128, i128)>) -> u64 {
    if intervals.is_empty() {
        return 0;
    }
    intervals.sort_by_key(|&(s, _)| s);
    let mut merged: Vec<(i128, i128)> = Vec::with_capacity(intervals.len());
    for iv in intervals {
        if let Some(last) = merged.last_mut()
            && iv.0 <= last.1
        {
            last.1 = last.1.max(iv.1);
        } else {
            merged.push(iv);
        }
    }
    let total: i128 = merged.iter().map(|&(s, e)| (e - s).max(0)).sum();
    if total < 0 { 0 } else { total as u64 }
}

fn u64_from_f64(v: f64) -> u64 {
    if !v.is_finite() || v < 0.0 {
        0
    } else {
        v as u64
    }
}

/// Parse an ISO-8601 timestamp (`2026-08-18T12:34:56.789Z` or
/// `…+00:00`) into `OffsetDateTime`. Returns `None` on any failure.
fn parse_iso_timestamp(input: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(input, &time::format_description::well_known::Rfc3339).ok()
}

/// Format duration between `first` and `last` matching TS
/// `getSessionDuration` output: `<1m`, `42m`, `3hr`, `1hr 12m`.
fn format_duration(first: Option<OffsetDateTime>, last: Option<OffsetDateTime>) -> Option<String> {
    let (first, last) = (first?, last?);
    let duration = last - first;
    let total_seconds = duration.whole_seconds();
    if total_seconds < 0 {
        return None;
    }
    let total_minutes = total_seconds / 60;
    if total_minutes < 1 {
        return Some("<1m".to_string());
    }
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    Some(match (hours, minutes) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}hr"),
        (h, m) => format!("{h}hr {m}m"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(std::path::PathBuf);
    impl TempFile {
        fn new(tag: &str, contents: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "glassline-tx-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0),
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("transcript.jsonl");
            std::fs::write(&path, contents).unwrap();
            Self(path)
        }
    }
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }

    fn all_needs() -> WidgetRequirements {
        WidgetRequirements::TRANSCRIPT
            | WidgetRequirements::COMPACTION
            | WidgetRequirements::SESSION_CLOCK
    }

    #[test]
    fn missing_file_returns_zeros() {
        let out = scan(
            std::path::Path::new("/definitely/does/not/exist.jsonl"),
            all_needs(),
        )
        .unwrap();
        assert_eq!(out, TranscriptScan::default());
    }

    #[test]
    fn empty_file_returns_zeros() {
        let f = TempFile::new("empty", "");
        let out = scan(&f.0, all_needs()).unwrap();
        assert_eq!(out.tokens, TokenMetrics::default());
        assert_eq!(out.compaction.count, 0);
        assert!(out.session_duration.is_none());
    }

    #[test]
    fn malformed_lines_are_skipped() {
        let f = TempFile::new("bad", "not json\n{}\n<broken>\n");
        let out = scan(&f.0, all_needs()).unwrap();
        assert_eq!(out, TranscriptScan::default());
    }

    #[test]
    fn token_sums_from_streamed_final_entries() {
        // Two-entry stream: first has stop_reason:null (partial), second
        // has stop_reason:"end_turn" (final). Only the final should count
        // when has_stop_reason_field is true, but per TS behavior the last
        // Null entry is also included as a live-update.
        let jsonl = "\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:00:00Z\",\"message\":{\"usage\":{\"input_tokens\":100,\"output_tokens\":50},\"stop_reason\":null}}\n\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:00:05Z\",\"message\":{\"usage\":{\"input_tokens\":100,\"output_tokens\":80},\"stop_reason\":\"end_turn\"}}\n";
        let f = TempFile::new("streamed", jsonl);
        let out = scan(&f.0, WidgetRequirements::TRANSCRIPT).unwrap();
        // Last Null gets included as live update per TS.
        // input = 100+100 = 200, output = 50+80 = 130.
        assert_eq!(out.tokens.input, 200);
        assert_eq!(out.tokens.output, 130);
    }

    #[test]
    fn context_length_ignores_pre_compaction_turn() {
        let jsonl = "\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:00:00Z\",\"message\":{\"usage\":{\"input_tokens\":150000,\"cache_read_input_tokens\":40000,\"output_tokens\":10}}}\n\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"timestamp\":\"2026-08-18T10:00:30Z\",\"compactMetadata\":{\"trigger\":\"auto\",\"preTokens\":190000,\"postTokens\":30000}}\n\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:01:00Z\",\"message\":{\"usage\":{\"input_tokens\":25000,\"cache_read_input_tokens\":5000,\"output_tokens\":20}}}\n";
        let f = TempFile::new("compact", jsonl);
        let out = scan(&f.0, WidgetRequirements::TRANSCRIPT).unwrap();
        // After compaction, the first post-boundary entry drives context.
        // 25000 + 5000 = 30000.
        assert_eq!(out.tokens.context_length, 30_000);
    }

    #[test]
    fn context_length_falls_back_to_boundary_post_tokens() {
        // Boundary present but no post-boundary usage entry — must fall
        // back to boundary.postTokens rather than the pre-boundary entry.
        let jsonl = "\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:00:00Z\",\"message\":{\"usage\":{\"input_tokens\":150000,\"output_tokens\":10}}}\n\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"timestamp\":\"2026-08-18T10:00:30Z\",\"compactMetadata\":{\"trigger\":\"manual\",\"preTokens\":180000,\"postTokens\":42000}}\n";
        let f = TempFile::new("bfallback", jsonl);
        let out = scan(&f.0, WidgetRequirements::TRANSCRIPT).unwrap();
        assert_eq!(out.tokens.context_length, 42_000);
    }

    #[test]
    fn sidechain_entries_dont_drive_context_length() {
        let jsonl = "\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:00:00Z\",\"message\":{\"usage\":{\"input_tokens\":10000,\"output_tokens\":10}}}\n\
{\"type\":\"assistant\",\"isSidechain\":true,\"timestamp\":\"2026-08-18T10:00:05Z\",\"message\":{\"usage\":{\"input_tokens\":80000,\"output_tokens\":10}}}\n";
        let f = TempFile::new("sidechain", jsonl);
        let out = scan(&f.0, WidgetRequirements::TRANSCRIPT).unwrap();
        assert_eq!(out.tokens.context_length, 10_000);
        // Sums include everything (matches TS aggregate behaviour).
        assert_eq!(out.tokens.input, 90_000);
    }

    #[test]
    fn compaction_stats_bucket_by_trigger() {
        let jsonl = "\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"compactMetadata\":{\"trigger\":\"auto\",\"preTokens\":100,\"postTokens\":40}}\n\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"compactMetadata\":{\"trigger\":\"manual\",\"preTokens\":200,\"postTokens\":60}}\n\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"compactMetadata\":{\"preTokens\":50,\"postTokens\":10}}\n";
        let f = TempFile::new("compact-buckets", jsonl);
        let out = scan(&f.0, WidgetRequirements::COMPACTION).unwrap();
        assert_eq!(out.compaction.count, 3);
        assert_eq!(out.compaction.by_trigger.auto, 1);
        assert_eq!(out.compaction.by_trigger.manual, 1);
        assert_eq!(out.compaction.by_trigger.unknown, 1);
        // 60 + 140 + 40 = 240.
        assert_eq!(out.compaction.tokens_reclaimed, 240);
    }

    #[test]
    fn compaction_ignores_sidechain_boundaries() {
        let jsonl = "\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"isSidechain\":true,\"compactMetadata\":{\"trigger\":\"auto\",\"preTokens\":100,\"postTokens\":10}}\n";
        let f = TempFile::new("sidechain-boundary", jsonl);
        let out = scan(&f.0, WidgetRequirements::COMPACTION).unwrap();
        assert_eq!(out.compaction.count, 0);
    }

    #[test]
    fn session_duration_computes_min_and_hour_form() {
        let jsonl = "\
{\"timestamp\":\"2026-08-18T10:00:00Z\"}\n\
{\"timestamp\":\"2026-08-18T10:42:30Z\"}\n";
        let f = TempFile::new("dur42m", jsonl);
        let out = scan(&f.0, WidgetRequirements::SESSION_CLOCK).unwrap();
        assert_eq!(out.session_duration.as_deref(), Some("42m"));

        let jsonl2 = "\
{\"timestamp\":\"2026-08-18T10:00:00Z\"}\n\
{\"timestamp\":\"2026-08-18T13:12:00Z\"}\n";
        let f2 = TempFile::new("dur3hr12m", jsonl2);
        let out2 = scan(&f2.0, WidgetRequirements::SESSION_CLOCK).unwrap();
        assert_eq!(out2.session_duration.as_deref(), Some("3hr 12m"));

        let jsonl3 = "\
{\"timestamp\":\"2026-08-18T10:00:00Z\"}\n\
{\"timestamp\":\"2026-08-18T10:00:20Z\"}\n";
        let f3 = TempFile::new("dur1m", jsonl3);
        let out3 = scan(&f3.0, WidgetRequirements::SESSION_CLOCK).unwrap();
        assert_eq!(out3.session_duration.as_deref(), Some("<1m"));
    }

    #[test]
    fn needs_gate_skips_unused_branches() {
        // No requirements = everything zero even with tokens on file.
        let jsonl = "\
{\"type\":\"assistant\",\"timestamp\":\"2026-08-18T10:00:00Z\",\"message\":{\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}}\n\
{\"type\":\"system\",\"subtype\":\"compact_boundary\",\"compactMetadata\":{\"trigger\":\"auto\",\"preTokens\":10,\"postTokens\":2}}\n";
        let f = TempFile::new("noneed", jsonl);
        let out = scan(&f.0, WidgetRequirements::NONE).unwrap();
        assert_eq!(out.tokens, TokenMetrics::default());
        assert_eq!(out.compaction.count, 0);
    }

    #[test]
    fn cached_and_total_helpers() {
        let m = TokenMetrics {
            input: 10,
            output: 5,
            cache_read: 20,
            cache_creation: 3,
            context_length: 40,
        };
        assert_eq!(m.cached(), 23);
        assert_eq!(m.total(), 38);
    }
}
