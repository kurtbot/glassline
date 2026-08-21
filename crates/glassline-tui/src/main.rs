//! `glassline-tui` — interactive layout config editor.
//!
//! P2 milestone: launches straight into [`WidgetPicker`] so the meta
//! catalog + drift test can be exercised end-to-end. P3 will wrap this
//! in a proper MainMenu → LineListEditor → ItemsEditor screen tree.

pub mod meta;
pub mod preview_ctx;
pub mod screens;

use std::path::PathBuf;

use glassline_core::settings::Settings;
use glassline_render::config::load;
use glassline_tui_dsl::{DslApp, DslError};

use crate::screens::MainMenu;

fn main() -> Result<(), DslError> {
    let (settings, path) = resolve_settings();
    let app = DslApp::new(Box::new(MainMenu::new()), settings, path);
    let outcome = app.run()?;
    println!("Editor exited: {outcome:?}");
    Ok(())
}

/// Resolve the settings.json path + initial scratch. Uses the same
/// path resolution as the render binary so what the editor writes is
/// what the hot path reads back.
fn resolve_settings() -> (Settings, PathBuf) {
    match load(None) {
        Ok(loaded) => (loaded.settings, loaded.path),
        Err(_) => (
            Settings::default(),
            PathBuf::from("./glassline-scratch.json"),
        ),
    }
}
