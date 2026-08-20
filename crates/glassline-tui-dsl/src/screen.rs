//! The [`Screen`] trait every editor screen implements, plus the
//! [`Action`] enum they return from `on_event` and the [`Outcome`] the
//! top-level app returns from `run`.
//!
//! Design reference: [[layout_config_editor_design_v1.1]] §4.3.

use ratatui::crossterm::event::Event;

use crate::ui::Ui;

/// Everything an editor screen implements. Screens are heap-boxed and
/// stacked inside [`crate::app::DslApp`]; only the top of the stack
/// receives `render` + `on_event` calls each tick.
pub trait Screen {
    /// Draw this screen into `ui`. Called on every tick + on every
    /// dirty-repaint request.
    fn render(&mut self, ui: &mut Ui);

    /// Handle one input event. Returns an [`Action`] the app applies to
    /// the screen stack (or `Action::None` to do nothing).
    fn on_event(&mut self, ev: Event) -> Action;

    /// Human-readable title. Rendered in the top chrome by the app.
    fn title(&self) -> &str;

    /// Local keybindings surfaced in the footer strip. Each tuple is
    /// `(key label, action label)` — e.g. `("Enter", "Edit")`. Screens
    /// return a `&'static` slice so the footer render allocates nothing.
    fn keybindings(&self) -> &[(&'static str, &'static str)];
}

/// Screens return an `Action` from `on_event` to drive the screen
/// stack. The app applies exactly one action per event.
pub enum Action {
    /// Do nothing — the screen absorbed the event or ignored it.
    None,
    /// Push a new screen on top of the stack.
    Push(Box<dyn Screen>),
    /// Pop the current screen. Popping the last screen exits the loop.
    Pop,
    /// Replace the current screen without changing stack depth.
    Replace(Box<dyn Screen>),
    /// Exit the app. `save = true` means "commit the scratch settings";
    /// `save = false` means "discard".
    Quit {
        save: bool,
    },
    /// Show a floating toast on the next repaint. Non-blocking — the
    /// current screen stays active.
    Toast(String),
}

/// What [`crate::app::DslApp::run`] returns to the caller once the
/// screen stack empties (or a screen returned `Action::Quit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// User committed their edits via `Save & Quit` or an equivalent.
    /// The app's atomic-save path already ran; caller has nothing left
    /// to do.
    Saved,
    /// User exited without saving (or the stack drained via `Pop` all
    /// the way to empty). Caller may want to prompt for confirmation
    /// before this happens; that lives in the screen, not here.
    Discarded,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Trivial screen that pops on any key press. Proves the trait's
    /// shape compiles and can be instantiated behind `Box<dyn Screen>`.
    struct HelloScreen;

    impl Screen for HelloScreen {
        fn render(&mut self, _ui: &mut Ui) {}
        fn on_event(&mut self, _ev: Event) -> Action {
            Action::Pop
        }
        fn title(&self) -> &str {
            "Hello"
        }
        fn keybindings(&self) -> &[(&'static str, &'static str)] {
            &[("Any", "Pop")]
        }
    }

    #[test]
    fn hello_screen_boxes_as_dyn_screen() {
        let boxed: Box<dyn Screen> = Box::new(HelloScreen);
        assert_eq!(boxed.title(), "Hello");
        assert_eq!(boxed.keybindings(), &[("Any", "Pop")]);
    }

    #[test]
    fn hello_screen_pops_on_any_event() {
        let mut screen = HelloScreen;
        let ev = Event::Key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        match screen.on_event(ev) {
            Action::Pop => {}
            _ => panic!("expected Action::Pop"),
        }
    }

    #[test]
    fn outcome_is_comparable() {
        assert_eq!(Outcome::Saved, Outcome::Saved);
        assert_ne!(Outcome::Saved, Outcome::Discarded);
    }
}
