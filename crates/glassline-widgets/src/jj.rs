//! Shell-out helpers for the Jujutsu (`jj`) widget family. Mirrors the
//! shape of [`crate::git`] — one `run_jj` cache per invocation, binary
//! probe in a `OnceLock<bool>`, and a `no_jj_short_circuit` helper the
//! widgets can use to bail cleanly outside a jj workspace.
//!
//! Every `jj log` invocation MUST include `--no-graph` — see
//! Without it jj decorates output
//! with graph glyphs (`@ │ ○ ◆`) that break parsers. All queries target
//! `-r @` (current change) as the convention.

use std::{
    collections::HashMap,
    process::Command,
    sync::{Mutex, OnceLock},
};

use glassline_core::{render_context::RenderContext, settings::WidgetSpec, span::StyledSpan};

use crate::git::resolve_git_cwd;

/// `+insertions -deletions files` summary from `jj diff --stat`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JjDiffStat {
    pub files: u32,
    pub insertions: u32,
    pub deletions: u32,
}

/// Whether `jj` is installed and on PATH. Cached in a `OnceLock` per
/// process — matches the [`crate::git::gh_available`] pattern.
#[must_use]
pub fn jj_available() -> bool {
    *JJ_AVAILABLE.get_or_init(|| {
        Command::new("jj")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

static JJ_AVAILABLE: OnceLock<bool> = OnceLock::new();

/// Cached `jj <args>` invocation. Returns `Some(stdout.trim())` on
/// success, or `None` on failure / missing jj / empty output. Cache is
/// per-invocation.
pub fn run_jj(args: &[&str], ctx: &RenderContext) -> Option<String> {
    if !jj_available() {
        return None;
    }
    let cwd = resolve_git_cwd(ctx)?;
    let key = format!("jj\0{cwd}\0{}", args.join("\0"));
    let cache = SHELL_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(v) = guard.get(&key)
    {
        return v.clone();
    }
    let output = Command::new("jj")
        .args(args)
        .current_dir(&cwd)
        .output()
        .ok();
    let result = match output {
        Some(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if text.is_empty() { None } else { Some(text) }
        }
        _ => None,
    };
    if let Ok(mut g) = cache.lock() {
        g.insert(key, result.clone());
    }
    result
}

static SHELL_CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

/// `jj root` succeeds inside a jj workspace. The result is cached with
/// the rest of the per-invocation `run_jj` output.
#[must_use]
pub fn is_inside_jj_workspace(ctx: &RenderContext) -> bool {
    run_jj(&["root"], ctx).is_some()
}

/// Shared "not in a jj workspace" short-circuit used by every jj widget.
///
/// Unlike git's placeholder (`(no jj)`), users only wire jj widgets into
/// their statusline when they're on a jj workflow — the widgets simply
/// render nothing outside a workspace. `metadata.showPlaceholder=true`
/// forces the `(no jj)` fallback for users who want an explicit marker.
#[must_use]
pub fn no_jj_short_circuit(spec: &WidgetSpec, ctx: &RenderContext) -> Option<Vec<StyledSpan>> {
    if is_inside_jj_workspace(ctx) {
        return None;
    }
    let show = spec
        .metadata
        .as_ref()
        .and_then(|m| m.get("showPlaceholder"))
        .is_some_and(|v| v == "true");
    Some(if show {
        crate::common::styled(spec, "(no jj)".into())
    } else {
        Vec::new()
    })
}

/// `jj root` -> workspace root directory.
#[must_use]
pub fn get_jj_root(ctx: &RenderContext) -> Option<String> {
    run_jj(&["root"], ctx)
}

/// Full commit_id of the current change (`@`) via
/// `jj log -r @ --no-graph -T commit_id`.
#[must_use]
pub fn get_jj_revision(ctx: &RenderContext) -> Option<String> {
    run_jj(&["log", "-r", "@", "--no-graph", "-T", "commit_id"], ctx)
}

/// Description of the current change, or empty string when unset.
#[must_use]
pub fn get_jj_description(ctx: &RenderContext) -> Option<String> {
    run_jj(
        &[
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "coalesce(description, \"\")",
        ],
        ctx,
    )
    .filter(|s| !s.is_empty())
}

/// Bookmarks pointing at the current change.
#[must_use]
pub fn get_jj_bookmarks(ctx: &RenderContext) -> Option<String> {
    run_jj(&["log", "-r", "@", "--no-graph", "-T", "bookmarks"], ctx).filter(|s| !s.is_empty())
}

/// Current workspace name from `jj workspace list`. Parses the first
/// line's `NAME:` prefix. Single-workspace repos return `default`.
#[must_use]
pub fn get_jj_workspace(ctx: &RenderContext) -> Option<String> {
    let raw = run_jj(&["workspace", "list"], ctx)?;
    let first = raw.lines().next()?.trim();
    let (name, _rest) = first.split_once(':')?;
    let n = name.trim();
    if n.is_empty() {
        None
    } else {
        Some(n.to_string())
    }
}

/// `jj diff --stat` summary. Same shape as `git diff --shortstat` and
/// parsed the same way (`extract_number` in git.rs).
#[must_use]
pub fn get_jj_diff_stat(ctx: &RenderContext) -> JjDiffStat {
    let raw = run_jj(&["diff", "--stat"], ctx).unwrap_or_default();
    parse_jj_diff_stat(&raw)
}

/// Parse a `jj diff --stat` summary line. Format matches `git diff
/// --shortstat`: `" N files changed, X insertions(+), Y deletions(-)"`.
pub(super) fn parse_jj_diff_stat(stat: &str) -> JjDiffStat {
    JjDiffStat {
        files: extract_number(stat, "file"),
        insertions: extract_number(stat, "insertion"),
        deletions: extract_number(stat, "deletion"),
    }
}

fn extract_number(stat: &str, keyword: &str) -> u32 {
    let idx = stat.find(keyword);
    let Some(pos) = idx else { return 0 };
    let bytes = stat.as_bytes();
    let mut end = pos;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && bytes[start - 1].is_ascii_digit() {
        start -= 1;
    }
    if start == end {
        return 0;
    }
    stat[start..end].parse().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stat_full_form() {
        let s = parse_jj_diff_stat(" 3 files changed, 42 insertions(+), 5 deletions(-)");
        assert_eq!(
            s,
            JjDiffStat {
                files: 3,
                insertions: 42,
                deletions: 5
            }
        );
    }

    #[test]
    fn parse_stat_singular() {
        let s = parse_jj_diff_stat(" 1 file changed, 1 insertion(+), 1 deletion(-)");
        assert_eq!(
            s,
            JjDiffStat {
                files: 1,
                insertions: 1,
                deletions: 1
            }
        );
    }

    #[test]
    fn parse_stat_insertions_only() {
        let s = parse_jj_diff_stat(" 2 files changed, 10 insertions(+)");
        assert_eq!(
            s,
            JjDiffStat {
                files: 2,
                insertions: 10,
                deletions: 0
            }
        );
    }

    #[test]
    fn parse_stat_empty() {
        assert_eq!(parse_jj_diff_stat(""), JjDiffStat::default());
    }
}
