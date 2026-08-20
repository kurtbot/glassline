//! Two-button confirmation modal. Screens push it when they need
//! yes/no consent (delete, discard-and-quit, replace-existing).

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Button, Modal, Screen, Ui};

/// One-shot callback: called with `true` on confirm, `false` on cancel.
type OnDecide = Box<dyn FnOnce(bool) -> Action>;

pub struct ConfirmModal {
    title_text: &'static str,
    body: String,
    selected: usize,
    on_decide: Option<OnDecide>,
}

impl ConfirmModal {
    pub fn new<F>(title: &'static str, body: impl Into<String>, on_decide: F) -> Self
    where
        F: FnOnce(bool) -> Action + 'static,
    {
        Self {
            title_text: title,
            body: body.into(),
            selected: 1, // default focus on "Cancel" — safer default
            on_decide: Some(Box::new(on_decide)),
        }
    }
}

const BUTTONS: [Button<'static>; 2] = [Button::new("OK"), Button::new("Cancel")];

impl Screen for ConfirmModal {
    fn title(&self) -> &str {
        self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("←/→", "Button"), ("Enter", "Activate"), ("Esc", "Cancel")]
    }
    fn render(&mut self, ui: &mut Ui) {
        // Dim backdrop first — draw a paragraph across the full frame
        // area so the modal reads as a foreground overlay.
        let area = ui.area();
        let [_top, main] =
            Layout::vertical([Constraint::Length(0), Constraint::Fill(1)]).areas(area);
        // Empty paragraph — allows the modal Clear to overwrite.
        ui.render_widget(Paragraph::new(""), main);
        Modal::new(self.title_text, &self.body, &BUTTONS)
            .with_selected(self.selected)
            .with_size(60, 30)
            .render(area, ui.frame);
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Left => {
                self.selected = self.selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Right => {
                self.selected = (self.selected + 1).min(BUTTONS.len() - 1);
                Action::None
            }
            KeyCode::Enter => {
                let confirmed = self.selected == 0;
                let cb = self
                    .on_decide
                    .take()
                    .expect("Enter fires the callback at most once");
                cb(confirmed)
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if let Some(cb) = self.on_decide.take() {
                    cb(false)
                } else {
                    Action::Pop
                }
            }
            _ => Action::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn enter_on_ok_fires_true() {
        let sink = Arc::new(Mutex::new(None::<bool>));
        let s = sink.clone();
        let mut modal = ConfirmModal::new("t", "body", move |ok| {
            *s.lock().unwrap() = Some(ok);
            Action::Pop
        });
        // Focus is default Cancel → move left to OK.
        modal.on_event(key(KeyCode::Left));
        modal.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(true));
    }

    #[test]
    fn enter_on_cancel_fires_false() {
        let sink = Arc::new(Mutex::new(None::<bool>));
        let s = sink.clone();
        let mut modal = ConfirmModal::new("t", "body", move |ok| {
            *s.lock().unwrap() = Some(ok);
            Action::Pop
        });
        // Default focus is Cancel.
        modal.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(false));
    }

    #[test]
    fn esc_fires_false() {
        let sink = Arc::new(Mutex::new(None::<bool>));
        let s = sink.clone();
        let mut modal = ConfirmModal::new("t", "body", move |ok| {
            *s.lock().unwrap() = Some(ok);
            Action::Pop
        });
        modal.on_event(key(KeyCode::Esc));
        assert_eq!(*sink.lock().unwrap(), Some(false));
    }
}
