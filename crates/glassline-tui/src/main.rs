//! `glassline-tui` — interactive layout config editor.
//!
//! Default (no args): launch the TUI against the resolved config path.
//! Non-interactive flags (`--import`, `--export`, `--dry-run`,
//! `--version`, `--help`) never enter the alt-screen and exit with
//! status 0 on success, 1 on failure. Scripted-upgrade friendly.

pub mod cli;
pub mod meta;
pub mod preview_ctx;
pub mod screens;
pub mod screenshots;

use std::process::ExitCode;

fn main() -> ExitCode {
    match cli::dispatch(std::env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("glassline-tui: {e}");
            ExitCode::FAILURE
        }
    }
}
