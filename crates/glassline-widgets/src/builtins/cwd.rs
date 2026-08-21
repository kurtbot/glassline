//! `current-working-dir` — displays the working directory, optionally
//! abbreviated. Port of TS `CurrentWorkingDir.tsx`.
//!
//! # Metadata
//!
//! - `abbreviateHome=true` — replace `$HOME` prefix with `~`.
//! - `fishStyle=true` — abbreviate every non-terminal segment to its
//!   first character (`/home/kurt/repos/glassline` → `/h/k/r/glassline`).
//!   Combines with `abbreviateHome`: `~/repos/glassline` → `~/r/glassline`.
//! - `segments=N` — keep only the last N path segments; longer paths get
//!   a leading `…/` prefix. `0` disables (default). Applied after
//!   `abbreviateHome` and `fishStyle`.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(CurrentWorkingDir)
}

pub struct CurrentWorkingDir;

impl Widget for CurrentWorkingDir {
    fn id(&self) -> &'static str {
        "current-working-dir"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }
    fn default_color(&self) -> Option<&'static str> {
        Some("blue")
    }

    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(data) = ctx.data.as_ref() else {
            return Vec::new();
        };
        let cwd = data
            .workspace
            .as_ref()
            .and_then(|w| w.current_dir.clone())
            .or_else(|| data.cwd.clone());
        let Some(cwd) = cwd else {
            return Vec::new();
        };
        let meta = |k: &str| spec.metadata.as_ref().and_then(|m| m.get(k)).cloned();
        let bool_of = |k: &str| meta(k).is_some_and(|v| v == "true");

        let mut text = if bool_of("abbreviateHome") {
            abbreviate_home(&cwd)
        } else {
            cwd
        };
        if bool_of("fishStyle") {
            text = fish_style(&text);
        }
        if let Some(n) = meta("segments").and_then(|v| v.parse::<usize>().ok())
            && n > 0
        {
            text = trim_to_last_segments(&text, n);
        }
        styled(spec, text)
    }
}

/// Abbreviate every non-terminal segment to its first character.
/// Preserves a leading `~` or drive letter. Non-ASCII segments take
/// their first Unicode scalar; hidden segments (starting with `.`)
/// keep the dot plus their first non-dot char (`.config` → `.c`).
fn fish_style(path: &str) -> String {
    // Detect the separator by looking at what's actually in the path.
    let sep = if path.contains('\\') && !path.contains('/') {
        '\\'
    } else {
        '/'
    };
    let parts: Vec<&str> = path.split(sep).collect();
    if parts.len() <= 1 {
        return path.to_string();
    }
    let last_idx = parts.len() - 1;
    let mut abbreviated: Vec<String> = Vec::with_capacity(parts.len());
    for (i, part) in parts.iter().enumerate() {
        if i == last_idx || part.is_empty() {
            abbreviated.push((*part).to_string());
            continue;
        }
        // Preserve `~`, drive letters (`C:`).
        if *part == "~" || part.ends_with(':') {
            abbreviated.push((*part).to_string());
            continue;
        }
        let head: String = if let Some(rest) = part.strip_prefix('.') {
            // `.config` → `.c`
            let mut s = String::from(".");
            if let Some(c) = rest.chars().next() {
                s.push(c);
            }
            s
        } else {
            part.chars().take(1).collect()
        };
        abbreviated.push(head);
    }
    abbreviated.join(&sep.to_string())
}

/// Keep only the last `n` path segments; longer paths get a leading
/// `…/` prefix. Uses the same separator inference as [`fish_style`].
fn trim_to_last_segments(path: &str, n: usize) -> String {
    let sep = if path.contains('\\') && !path.contains('/') {
        '\\'
    } else {
        '/'
    };
    let parts: Vec<&str> = path.split(sep).collect();
    if parts.len() <= n {
        return path.to_string();
    }
    let tail = &parts[parts.len() - n..];
    format!("\u{2026}{sep}{}", tail.join(&sep.to_string()))
}

fn abbreviate_home(path: &str) -> String {
    let Some(home) = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .and_then(|s| s.into_string().ok())
    else {
        return path.to_string();
    };
    if home.is_empty() {
        return path.to_string();
    }
    // Compare case-insensitively on Windows so `C:\Users\Foo` matches
    // `c:\users\foo`, matching TS behaviour via os.homedir().
    let (path_cmp, home_cmp) = if cfg!(windows) {
        (path.to_ascii_lowercase(), home.to_ascii_lowercase())
    } else {
        (path.to_string(), home.clone())
    };
    if path_cmp == home_cmp {
        return "~".to_string();
    }
    if let Some(rest) = path_cmp.strip_prefix(&home_cmp) {
        // Only strip when it's a real prefix followed by a separator, so
        // `/home/foobar` doesn't get eaten by `/home/foo`.
        if rest.starts_with('/') || rest.starts_with('\\') {
            let real_rest = &path[home.len()..];
            return format!("~{real_rest}");
        }
    }
    path.to_string()
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
    fn workspace_current_dir_wins_over_top_cwd() {
        let ctx = RenderContext {
            data: Some(StatusJson {
                cwd: Some("/wrong".into()),
                workspace: Some(Workspace {
                    current_dir: Some("/right".into()),
                    project_dir: None,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let spans = CurrentWorkingDir.render(&WidgetSpec::new("1", "current-working-dir"), &ctx);
        assert_eq!(spans[0].text, "/right");
    }

    #[test]
    fn falls_back_to_top_cwd_when_no_workspace() {
        let spans = CurrentWorkingDir.render(
            &WidgetSpec::new("1", "current-working-dir"),
            &ctx_with_cwd("/proj"),
        );
        assert_eq!(spans[0].text, "/proj");
    }

    #[test]
    fn empty_when_no_data() {
        let spans = CurrentWorkingDir.render(
            &WidgetSpec::new("1", "current-working-dir"),
            &RenderContext::default(),
        );
        assert!(spans.is_empty());
    }

    #[test]
    fn fish_style_shortens_all_but_last_segment() {
        assert_eq!(fish_style("/home/kurt/repos/glassline"), "/h/k/r/glassline");
    }

    #[test]
    fn fish_style_preserves_tilde() {
        assert_eq!(fish_style("~/repos/glassline"), "~/r/glassline");
    }

    #[test]
    fn fish_style_handles_hidden_segments() {
        assert_eq!(
            fish_style("/home/kurt/.config/glassline"),
            "/h/k/.c/glassline"
        );
    }

    #[test]
    fn fish_style_preserves_drive_letter_windows() {
        assert_eq!(
            fish_style("C:\\Users\\kurt\\repos\\glassline"),
            "C:\\U\\k\\r\\glassline"
        );
    }

    #[test]
    fn fish_style_no_op_on_single_segment() {
        assert_eq!(fish_style("glassline"), "glassline");
    }

    #[test]
    fn trim_to_last_segments_shortens_when_longer() {
        assert_eq!(
            trim_to_last_segments("/home/kurt/repos/glassline", 2),
            "\u{2026}/repos/glassline"
        );
    }

    #[test]
    fn trim_to_last_segments_no_op_when_shorter() {
        assert_eq!(trim_to_last_segments("/home/kurt", 5), "/home/kurt");
    }

    #[test]
    fn trim_to_last_segments_windows_path() {
        assert_eq!(
            trim_to_last_segments("C:\\Users\\kurt\\repos\\glassline", 2),
            "\u{2026}\\repos\\glassline"
        );
    }

    #[test]
    fn abbreviate_home_prefix() {
        let _guard = crate::common::TEST_ENV_LOCK.lock().unwrap();
        // Snapshot + restore so we don't clobber another test's HOME.
        let saved_home = std::env::var_os("HOME");
        let saved_userprofile = std::env::var_os("USERPROFILE");
        unsafe {
            std::env::set_var("HOME", "/home/user");
            // Windows path resolution also consults USERPROFILE; clear it
            // so this test is deterministic regardless of OS.
            std::env::remove_var("USERPROFILE");
        }
        let mut spec = WidgetSpec::new("1", "current-working-dir");
        spec.metadata = Some(
            [("abbreviateHome".to_string(), "true".to_string())]
                .into_iter()
                .collect(),
        );
        let spans = CurrentWorkingDir.render(&spec, &ctx_with_cwd("/home/user/proj"));
        assert_eq!(spans[0].text, "~/proj");
        unsafe {
            match saved_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match saved_userprofile {
                Some(v) => std::env::set_var("USERPROFILE", v),
                None => std::env::remove_var("USERPROFILE"),
            }
        }
    }
}
