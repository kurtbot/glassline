//! The top-level app: holds the screen stack, the shared scratch
//! [`Settings`], and drives the ratatui event loop.

use std::{path::PathBuf, time::Duration};

use ratatui::{
    Terminal,
    backend::Backend,
    crossterm::event::{self, Event},
};
use thiserror::Error;

use glassline_core::settings::Settings;

use crate::screen::{Action, Outcome, Screen};
use crate::ui::Ui;

/// Errors DslApp surfaces to its caller. Terminal I/O and event polling
/// both funnel into `Io`. Callers that just want a `Result<(), _>` can
/// map this straight through.
#[derive(Debug, Error)]
pub enum DslError {
    #[error("terminal I/O failed: {0}")]
    Io(#[from] std::io::Error),
    /// Any error surfaced by the ratatui backend. Backends define their
    /// own error type (crossterm uses `std::io::Error`, test backend
    /// uses `Infallible` in some versions), so we box.
    #[error("backend failure: {0}")]
    Backend(Box<dyn std::error::Error + Send + Sync>),
    #[error("screen stack drained without an explicit Quit")]
    UnexpectedEmpty,
}

/// The interactive TUI app. Owns the screen stack, the shared scratch
/// [`Settings`] every screen mutates, dirty tracking, and where the
/// final save writes.
pub struct DslApp {
    screens: Vec<Box<dyn Screen>>,
    tick_rate: Duration,
    scratch: Settings,
    dirty: bool,
    committed_path: PathBuf,
    pending_toast: Option<String>,
}

impl DslApp {
    /// Build a new app with `root` at the bottom of the screen stack.
    /// `committed_path` names where a `Save` action will write.
    #[must_use]
    pub fn new(root: Box<dyn Screen>, scratch: Settings, committed_path: PathBuf) -> Self {
        Self {
            screens: vec![root],
            tick_rate: Duration::from_millis(100),
            scratch,
            dirty: false,
            committed_path,
            pending_toast: None,
        }
    }

    /// Override the event-poll tick rate. Default is 100 ms. Lower =
    /// snappier repaints on animated content, higher = less CPU.
    pub fn with_tick_rate(mut self, tick_rate: Duration) -> Self {
        self.tick_rate = tick_rate;
        self
    }

    /// Read-only accessor for tests + Diagnostics.
    #[must_use]
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Read-only accessor for tests + Diagnostics.
    #[must_use]
    pub fn scratch(&self) -> &Settings {
        &self.scratch
    }

    /// Read-only accessor for the resolved save path.
    #[must_use]
    pub fn committed_path(&self) -> &std::path::Path {
        &self.committed_path
    }

    /// Read-only accessor for the current screen-stack depth. Useful
    /// for tests + Diagnostics.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    /// Run the app to completion using a real crossterm-backed terminal.
    /// The terminal is set up + torn down + panic-hooked automatically
    /// via `ratatui::run`.
    ///
    /// # Errors
    /// Returns [`DslError::Io`] on terminal I/O failure.
    pub fn run(mut self) -> Result<Outcome, DslError> {
        ratatui::run(|terminal| self.event_loop(terminal))
    }

    /// The event loop, generic over the backend so tests can drive a
    /// [`ratatui::backend::TestBackend`]. Blocks on `event::read()` when
    /// `event::poll` reports an event within `tick_rate`.
    ///
    /// # Errors
    /// Returns [`DslError::Io`] or [`DslError::Backend`] on any backend
    /// or event-poll failure.
    pub fn event_loop<B>(&mut self, terminal: &mut Terminal<B>) -> Result<Outcome, DslError>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        loop {
            self.draw(terminal)?;
            if self.screens.is_empty() {
                return Ok(Outcome::Discarded);
            }
            if event::poll(self.tick_rate)? {
                let ev = event::read()?;
                if let Some(outcome) = self.step_event(ev) {
                    return Ok(outcome);
                }
            }
        }
    }

    /// Draw the top screen. Called from the event loop and directly
    /// from tests that don't want to spin up a poll loop.
    ///
    /// # Errors
    /// Returns [`DslError::Backend`] on backend failure.
    pub fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> Result<(), DslError>
    where
        B: Backend,
        B::Error: std::error::Error + Send + Sync + 'static,
    {
        if let Some(top) = self.screens.last_mut() {
            let scratch = &self.scratch;
            terminal
                .draw(|frame| {
                    let mut ui = Ui::new(frame, scratch);
                    top.render(&mut ui);
                })
                .map_err(|e| DslError::Backend(Box::new(e)))?;
        }
        Ok(())
    }

    /// Feed one event to the top screen and apply the returned action
    /// to the screen stack. Returns `Some(outcome)` when the app should
    /// stop (Quit or empty stack). Test entry point — production code
    /// goes through [`Self::event_loop`].
    pub fn step_event(&mut self, ev: Event) -> Option<Outcome> {
        let Some(top) = self.screens.last_mut() else {
            return Some(Outcome::Discarded);
        };
        let action = top.on_event(ev);
        self.apply(action)
    }

    fn apply(&mut self, action: Action) -> Option<Outcome> {
        match action {
            Action::None => None,
            Action::Push(screen) => {
                self.screens.push(screen);
                None
            }
            Action::Pop => {
                self.screens.pop();
                if self.screens.is_empty() {
                    Some(Outcome::Discarded)
                } else {
                    None
                }
            }
            Action::Replace(screen) => {
                self.screens.pop();
                self.screens.push(screen);
                None
            }
            Action::Quit { save } => Some(if save {
                Outcome::Saved
            } else {
                Outcome::Discarded
            }),
            Action::Toast(text) => {
                self.pending_toast = Some(text);
                None
            }
        }
    }

    /// Mark the scratch settings as dirty. Screens call this when they
    /// mutate `scratch` so Quit-if-dirty can prompt.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear the pending toast (called by the app after painting).
    /// Returns the toast text if one was queued.
    pub fn take_toast(&mut self) -> Option<String> {
        self.pending_toast.take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// Test screen: pops on `p`, pushes another `PopOnP` on `d`, quits
    /// (Save=true) on `q`, otherwise no-op.
    struct PopOnP;
    impl Screen for PopOnP {
        fn render(&mut self, _ui: &mut Ui) {}
        fn on_event(&mut self, ev: Event) -> Action {
            let Event::Key(k) = ev else {
                return Action::None;
            };
            match k.code {
                KeyCode::Char('p') => Action::Pop,
                KeyCode::Char('d') => Action::Push(Box::new(PopOnP)),
                KeyCode::Char('q') => Action::Quit { save: true },
                KeyCode::Char('t') => Action::Toast("hi".into()),
                _ => Action::None,
            }
        }
        fn title(&self) -> &str {
            "PopOnP"
        }
        fn keybindings(&self) -> &[(&'static str, &'static str)] {
            &[("p", "Pop"), ("d", "Push"), ("q", "Save & Quit")]
        }
    }

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    fn app_with_root() -> DslApp {
        DslApp::new(
            Box::new(PopOnP),
            Settings::default(),
            PathBuf::from("/tmp/x.json"),
        )
    }

    #[test]
    fn depth_starts_at_one() {
        assert_eq!(app_with_root().depth(), 1);
    }

    #[test]
    fn push_increases_depth() {
        let mut app = app_with_root();
        assert!(app.step_event(key('d')).is_none());
        assert_eq!(app.depth(), 2);
    }

    #[test]
    fn pop_last_screen_returns_discarded() {
        let mut app = app_with_root();
        let outcome = app.step_event(key('p'));
        assert_eq!(outcome, Some(Outcome::Discarded));
        assert_eq!(app.depth(), 0);
    }

    #[test]
    fn pop_intermediate_screen_stays_running() {
        let mut app = app_with_root();
        app.step_event(key('d')); // depth 2
        app.step_event(key('d')); // depth 3
        assert_eq!(app.depth(), 3);
        let outcome = app.step_event(key('p'));
        assert!(outcome.is_none());
        assert_eq!(app.depth(), 2);
    }

    #[test]
    fn quit_save_true_returns_saved() {
        let mut app = app_with_root();
        assert_eq!(app.step_event(key('q')), Some(Outcome::Saved));
    }

    #[test]
    fn toast_action_stashes_text() {
        let mut app = app_with_root();
        assert!(app.take_toast().is_none());
        app.step_event(key('t'));
        assert_eq!(app.take_toast().as_deref(), Some("hi"));
        // Draining leaves the slot empty for the next paint cycle.
        assert!(app.take_toast().is_none());
    }

    #[test]
    fn mark_dirty_flips_flag() {
        let mut app = app_with_root();
        assert!(!app.dirty());
        app.mark_dirty();
        assert!(app.dirty());
    }

    #[test]
    fn draw_against_test_backend_does_not_panic() {
        let backend = TestBackend::new(20, 3);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = app_with_root();
        app.draw(&mut term)
            .expect("draw must not fail on TestBackend");
    }

    #[test]
    fn ignored_key_does_not_change_depth() {
        let mut app = app_with_root();
        let d0 = app.depth();
        app.step_event(key('x'));
        assert_eq!(app.depth(), d0);
    }

    #[test]
    fn committed_path_accessor_reads_back() {
        let app = DslApp::new(
            Box::new(PopOnP),
            Settings::default(),
            PathBuf::from("/somewhere/settings.json"),
        );
        assert_eq!(
            app.committed_path(),
            std::path::Path::new("/somewhere/settings.json")
        );
    }
}
