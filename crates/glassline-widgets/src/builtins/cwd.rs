//! `current-working-dir` — displays the working directory, optionally
//! abbreviated to `~`. Port of TS `CurrentWorkingDir.tsx`.
//!
//! **Deferred:** `fish-style` compression + `segments: N` truncation — those
//! rely on OS-specific home-dir detection + slash-split logic that add ~150
//! LOC. The MVP handles the abbreviateHome case; segments/fish arrive with
//! T-1.7b if any of the user's fixtures actually set them.

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
        let abbreviate = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("abbreviateHome"))
            .is_some_and(|v| v == "true");
        let text = if abbreviate {
            abbreviate_home(&cwd)
        } else {
            cwd
        };
        styled(spec, text)
    }
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
