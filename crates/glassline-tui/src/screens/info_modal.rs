//! Single-button confirmation dialog for one-shot outcomes. Unlike
//! [`super::confirm_modal::ConfirmModal`] (yes/no), this exists purely
//! to force the user to acknowledge a message before dismissal —
//! toast can vanish before the eye lands on it; a modal blocks.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Button, Modal, Screen, Ui};

pub struct InfoModal {
    title_text: String,
    body: String,
}

impl InfoModal {
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title_text: title.into(),
            body: body.into(),
        }
    }
}

const BUTTONS: [Button<'static>; 1] = [Button::new("OK")];

impl Screen for InfoModal {
    fn title(&self) -> &str {
        &self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("Enter / Esc", "Dismiss")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.frame_area();
        let [_top, main] =
            Layout::vertical([Constraint::Length(0), Constraint::Fill(1)]).areas(area);
        // Blank canvas beneath so the modal Clear can overwrite.
        ui.render_widget(Paragraph::new(""), main);
        Modal::new(&self.title_text, &self.body, &BUTTONS)
            .with_size(60, 30)
            .render(area, ui.frame);
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
            _ => Action::None,
        }
    }
}
