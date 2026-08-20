//! `git-branch` — current branch name, or a per-user fallback on detached HEAD.
//! Port of TS `GitBranch.tsx` render path. MVP: no hyperlink, no max-width
//! truncation, no symbol prefix override.
//!
//! **Detached HEAD:** by default the widget renders the short SHA (matches
//! shell-prompt convention in starship / powerlevel10k / zsh vcs_info).
//! Override via `WidgetSpec.metadata.detachedFallback`:
//! - `"sha"` (default) — short SHA of HEAD, or empty if git can't produce one.
//! - `"text"` — literal `(detached)`.
//! - `"empty"` — hide the widget entirely, letting separators collapse.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::{
    common::styled,
    git::{get_git_branch, get_git_short_sha, no_git_short_circuit},
};

pub fn factory() -> Box<dyn Widget> {
    Box::new(GitBranch)
}

pub struct GitBranch;

impl Widget for GitBranch {
    fn id(&self) -> &'static str {
        "git-branch"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::GIT
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("magenta")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        if let Some(early) = no_git_short_circuit(spec, ctx) {
            return early;
        }
        let text = get_git_branch(ctx).unwrap_or_else(|| detached_fallback(spec, ctx));
        if text.is_empty() {
            return Vec::new();
        }
        styled(spec, text)
    }
}

fn detached_fallback(spec: &WidgetSpec, ctx: &RenderContext) -> String {
    let mode = spec
        .metadata
        .as_ref()
        .and_then(|m| m.get("detachedFallback"))
        .map(String::as_str)
        .unwrap_or("sha");
    match mode {
        "text" => "(detached)".to_string(),
        "empty" => String::new(),
        // "sha" and any unrecognised value fall through to the SHA path.
        _ => get_git_short_sha(ctx).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests here exercise only the pure branches of `detached_fallback`
    //! (the `"text"` and `"empty"` modes, plus the unknown-mode fallthrough).
    //! The `"sha"` path shells out to real `git`, so it's covered by
    //! integration + manual smoke — same convention as `git_sha.rs` and the
    //! rest of the git widget family.

    use super::*;
    use std::collections::BTreeMap;

    fn spec_with_metadata(pairs: &[(&str, &str)]) -> WidgetSpec {
        let mut s = WidgetSpec::new("1", "git-branch");
        s.metadata = Some(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect::<BTreeMap<String, String>>(),
        );
        s
    }

    #[test]
    fn detached_fallback_text_mode_returns_literal() {
        let spec = spec_with_metadata(&[("detachedFallback", "text")]);
        let text = detached_fallback(&spec, &RenderContext::default());
        assert_eq!(text, "(detached)");
    }

    #[test]
    fn detached_fallback_empty_mode_returns_empty_string() {
        let spec = spec_with_metadata(&[("detachedFallback", "empty")]);
        let text = detached_fallback(&spec, &RenderContext::default());
        assert!(text.is_empty());
    }

    #[test]
    fn detached_fallback_sha_mode_returns_empty_when_no_git() {
        // No cwd → run_git bails → get_git_short_sha returns None → helper
        // yields an empty string. The render() caller then converts that to
        // Vec::new() so the widget hides.
        let spec = spec_with_metadata(&[("detachedFallback", "sha")]);
        let text = detached_fallback(&spec, &RenderContext::default());
        assert!(text.is_empty());
    }

    #[test]
    fn detached_fallback_default_when_metadata_missing_is_sha() {
        // Same behaviour as "sha" mode above — empty string without git,
        // proving the default path routes through get_git_short_sha.
        let spec = WidgetSpec::new("1", "git-branch");
        let text = detached_fallback(&spec, &RenderContext::default());
        assert!(text.is_empty());
    }

    #[test]
    fn detached_fallback_unknown_mode_treats_as_sha() {
        // Unrecognised value must not render as literal — it falls through
        // to the sha path, which returns empty when git is unavailable.
        let spec = spec_with_metadata(&[("detachedFallback", "chunky-bacon")]);
        let text = detached_fallback(&spec, &RenderContext::default());
        assert!(text.is_empty());
    }
}
