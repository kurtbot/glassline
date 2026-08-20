//! `glassline-tui` — interactive layout config editor.
//!
//! P2 scaffold: the binary compiles + prints a build-marker so packaging
//! can wire it up. Real screens land in P3.

fn main() {
    eprintln!(
        "glassline-tui {} — under construction",
        env!("CARGO_PKG_VERSION")
    );
    eprintln!(
        "The interactive TUI editor is under development. Configure via \
         %APPDATA%/glassline/settings.json (or ~/.config/glassline/settings.json) for now."
    );
}

pub mod meta;
pub mod preview_ctx;
pub mod screens;
