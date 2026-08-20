//! `glassline-tui` — interactive layout config editor.
//!
//! P2 milestone: launches straight into [`WidgetPicker`] so the meta
//! catalog + drift test can be exercised end-to-end. P3 will wrap this
//! in a proper MainMenu → LineListEditor → ItemsEditor screen tree.

pub mod meta;
pub mod preview_ctx;
pub mod screens;

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use glassline_core::settings::Settings;
use glassline_tui_dsl::{Action, DslApp, DslError};

use crate::screens::WidgetPicker;

fn main() -> Result<(), DslError> {
    let chosen: Arc<Mutex<Option<&'static str>>> = Arc::new(Mutex::new(None));

    let picker_chosen = Arc::clone(&chosen);
    let picker = WidgetPicker::new(move |meta| {
        *picker_chosen.lock().unwrap() = Some(meta.id);
        Action::Pop
    });

    let app = DslApp::new(
        Box::new(picker),
        Settings::default(),
        PathBuf::from("./glassline-scratch.json"),
    );
    let outcome = app.run()?;

    // Print after ratatui restores the terminal.
    match *chosen.lock().unwrap() {
        Some(id) => println!("You picked: {id}  (outcome: {outcome:?})"),
        None => println!("No widget picked  (outcome: {outcome:?})"),
    }
    Ok(())
}
