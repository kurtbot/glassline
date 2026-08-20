//! Shell-out helpers for the git widget family. Port of `utils/git.ts`.
//!
//! **Caching:** per-invocation only. `OnceLock<Mutex<HashMap>>` caches
//! command results so a single line with `git-branch` + `git-changes` +
//! `git-status` runs each git command at most once. Disk cache (design §4.8
//! parity with TS `getGitReviewCache`) is deferred to T-1.7c.
//!
//! **No `git2` / `gix`:** design §4.8 explicitly shells out to `git` to
//! preserve TS behaviour + keep the binary under budget.

use std::{
    collections::HashMap,
    process::Command,
    sync::{Mutex, OnceLock},
};

use glassline_core::{render_context::RenderContext, settings::WidgetSpec, span::StyledSpan};

use crate::common::styled;

/// Rendered value for a `git diff --shortstat` line: `+insertions -deletions`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GitChangeCounts {
    pub insertions: u32,
    pub deletions: u32,
}

/// Flags parsed from `git status --porcelain -z`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GitStatus {
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
    pub conflicts: bool,
}

/// Resolve the directory for a git shell-out.
///
/// Precedence matches TS `resolveGitCwd`: `data.cwd` > `workspace.current_dir`
/// > `workspace.project_dir`. Empty/whitespace entries are skipped.
#[must_use]
pub fn resolve_git_cwd(ctx: &RenderContext) -> Option<String> {
    let data = ctx.data.as_ref()?;
    let candidates = [
        data.cwd.as_deref(),
        data.workspace
            .as_ref()
            .and_then(|w| w.current_dir.as_deref()),
        data.workspace
            .as_ref()
            .and_then(|w| w.project_dir.as_deref()),
    ];
    for c in candidates {
        if let Some(s) = c
            && !s.trim().is_empty()
        {
            return Some(s.to_string());
        }
    }
    None
}

/// Cached `git <args>` invocation. Returns `Some(stdout.trim())` on
/// success, or `None` on failure, missing cwd, or empty output. Cache is
/// per-invocation.
pub fn run_git(args: &[&str], ctx: &RenderContext) -> Option<String> {
    let cwd = resolve_git_cwd(ctx)?;
    let key = format!("{cwd}\0{}", args.join("\0"));
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(guard) = cache.lock()
        && let Some(v) = guard.get(&key)
    {
        return v.clone();
    }

    let output = Command::new("git")
        .args(args)
        .current_dir(&cwd)
        .env("GIT_OPTIONAL_LOCKS", "0")
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

static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

/// `git rev-parse --is-inside-work-tree` -> `true`.
#[must_use]
pub fn is_inside_git_work_tree(ctx: &RenderContext) -> bool {
    run_git(&["rev-parse", "--is-inside-work-tree"], ctx).as_deref() == Some("true")
}

/// Shared "not in a git repo" short-circuit used by every git widget.
///
/// Returns:
/// - `Some(styled("(no git)"))` — outside a repo, widget should render the placeholder.
/// - `Some(Vec::new())` — outside a repo AND `metadata.hideNoGit == "true"`; widget renders nothing.
/// - `None` — inside a repo; widget should proceed with its own rendering.
#[must_use]
pub fn no_git_short_circuit(spec: &WidgetSpec, ctx: &RenderContext) -> Option<Vec<StyledSpan>> {
    if is_inside_git_work_tree(ctx) {
        return None;
    }
    let hide = spec
        .metadata
        .as_ref()
        .and_then(|m| m.get("hideNoGit"))
        .is_some_and(|v| v == "true");
    Some(if hide {
        Vec::new()
    } else {
        styled(spec, "(no git)".into())
    })
}

/// Current branch name via `rev-parse --abbrev-ref HEAD`. Returns `None`
/// on detached HEAD (git prints `HEAD`) or when the command fails.
#[must_use]
pub fn get_git_branch(ctx: &RenderContext) -> Option<String> {
    let raw = run_git(&["rev-parse", "--abbrev-ref", "HEAD"], ctx)?;
    if raw == "HEAD" { None } else { Some(raw) }
}

/// Short SHA of `HEAD` via `rev-parse --short HEAD`.
#[must_use]
pub fn get_git_short_sha(ctx: &RenderContext) -> Option<String> {
    run_git(&["rev-parse", "--short", "HEAD"], ctx)
}

/// Absolute path of the git repo root via `rev-parse --show-toplevel`.
#[must_use]
pub fn get_git_root(ctx: &RenderContext) -> Option<String> {
    run_git(&["rev-parse", "--show-toplevel"], ctx)
}

/// `git diff --shortstat` + `git diff --cached --shortstat` combined.
#[must_use]
pub fn get_git_change_counts(ctx: &RenderContext) -> GitChangeCounts {
    let unstaged = run_git(&["diff", "--shortstat"], ctx).unwrap_or_default();
    let staged = run_git(&["diff", "--cached", "--shortstat"], ctx).unwrap_or_default();
    let a = parse_shortstat(&unstaged);
    let b = parse_shortstat(&staged);
    GitChangeCounts {
        insertions: a.insertions + b.insertions,
        deletions: a.deletions + b.deletions,
    }
}

fn parse_shortstat(stat: &str) -> GitChangeCounts {
    let insertions = extract_number(stat, "insertion");
    let deletions = extract_number(stat, "deletion");
    GitChangeCounts {
        insertions,
        deletions,
    }
}

fn extract_number(stat: &str, keyword: &str) -> u32 {
    // Format: " N insertions(+)" or " N insertion(+)" — number precedes
    // the keyword after whitespace.
    let idx = stat.find(keyword);
    let Some(pos) = idx else { return 0 };
    // Walk backwards for digits.
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

/// `git status --porcelain -z` -> flags.
#[must_use]
pub fn get_git_status(ctx: &RenderContext) -> GitStatus {
    let Some(output) = run_git(&["status", "--porcelain", "-z"], ctx) else {
        return GitStatus::default();
    };
    let mut status = GitStatus::default();
    let entries: Vec<&str> = output.split('\0').collect();
    let mut i = 0;
    while i < entries.len() {
        let line = entries[i];
        i += 1;
        if line.len() < 2 {
            continue;
        }
        let bytes = line.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;

        // Conflict markers: DD/AU/UD/UA/DU/AA/UU.
        if !status.conflicts
            && matches!(
                (x, y),
                ('D', 'D')
                    | ('A', 'U')
                    | ('U', 'D')
                    | ('U', 'A')
                    | ('D', 'U')
                    | ('A', 'A')
                    | ('U', 'U')
            )
        {
            status.conflicts = true;
        }
        if !status.staged && matches!(x, 'M' | 'A' | 'D' | 'R' | 'C' | 'T' | 'U') {
            status.staged = true;
        }
        if !status.unstaged && matches!(y, 'M' | 'A' | 'D' | 'R' | 'C' | 'T' | 'U') {
            status.unstaged = true;
        }
        if !status.untracked && x == '?' && y == '?' {
            status.untracked = true;
        }
        if status.staged && status.unstaged && status.untracked && status.conflicts {
            break;
        }
        // Renames/copies have a second entry (old path) that we skip.
        if x == 'R' || x == 'C' || y == 'R' || y == 'C' {
            i += 1;
        }
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use glassline_core::status_json::{StatusJson, Workspace};

    fn ctx_with_cwd(cwd: &str) -> RenderContext {
        RenderContext {
            data: Some(StatusJson {
                cwd: Some(cwd.into()),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_prefers_data_cwd() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: Some("/data".into()),
                workspace: Some(Workspace {
                    current_dir: Some("/ws".into()),
                    project_dir: Some("/proj".into()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(resolve_git_cwd(&ctx).as_deref(), Some("/data"));
    }

    #[test]
    fn resolve_falls_through_workspace() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: None,
                workspace: Some(Workspace {
                    current_dir: Some("   ".into()),
                    project_dir: Some("/proj".into()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(resolve_git_cwd(&ctx).as_deref(), Some("/proj"));
    }

    #[test]
    fn resolve_returns_none_when_all_empty() {
        let ctx = RenderContext {
            data: Some(StatusJson::default()),
            ..Default::default()
        };
        assert!(resolve_git_cwd(&ctx).is_none());
    }

    #[test]
    fn parse_shortstat_handles_singular_and_plural() {
        let a = parse_shortstat(" 1 file changed, 42 insertions(+), 10 deletions(-)");
        assert_eq!(
            a,
            GitChangeCounts {
                insertions: 42,
                deletions: 10
            }
        );
        let b = parse_shortstat(" 1 file changed, 1 insertion(+), 1 deletion(-)");
        assert_eq!(
            b,
            GitChangeCounts {
                insertions: 1,
                deletions: 1
            }
        );
        let c = parse_shortstat("");
        assert_eq!(c, GitChangeCounts::default());
    }

    #[test]
    fn is_inside_work_tree_smoke() {
        // We can't rely on git being available in every CI runner; just
        // exercise the code path against a bogus cwd and confirm the
        // function returns cleanly.
        let ctx = ctx_with_cwd("C:\\ThisPathDefinitelyDoesNotExist_glassline");
        let _ = is_inside_git_work_tree(&ctx);
    }
}
