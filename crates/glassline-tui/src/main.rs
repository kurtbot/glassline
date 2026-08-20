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
use glassline_tui_dsl::{DslApp, DslError};

use crate::screens::MainMenu;

fn main() -> Result<(), DslError> {
    let app = DslApp::new(
        Box::new(MainMenu::new()),
        Settings::default(),
        PathBuf::from("./glassline-scratch.json"),
    );
    let outcome = app.run()?;
    println!("Editor exited: {outcome:?}");
    Ok(())
}
