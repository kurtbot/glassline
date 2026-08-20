//! Temporary screen used by [`super::main_menu::MainMenu`] entries
//! whose real screen hasn't landed yet. Renders a title + body panel
//! and pops on Esc.
//!
//! Removed as each phase fills in the corresponding real screen.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Panel, Screen, Ui};

pub struct Placeholder {
    title_text: String,
    body: String,
}

impl Placeholder {
    #[must_use]
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title_text: title.into(),
            body: body.into(),
        }
    }
}

impl Screen for Placeholder {
    fn title(&self) -> &str {
        &self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("Esc", "Back")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let [_top, main, _footer] = Layout::vertical([
            Constraint::Length(0),
            Constraint::Fill(1),
            Constraint::Length(0),
        ])
        .areas(area);
        Panel::new(&self.title_text).render(main, ui.frame, |inner, frame| {
            frame.render_widget(Paragraph::new(self.body.as_str()), inner);
        });
    }
    fn on_event(&mut self, ev: Event) -> Action {
        if let Event::Key(k) = ev
            && matches!(k.code, KeyCode::Esc | KeyCode::Char('q'))
        {
            return Action::Pop;
        }
        Action::None
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn esc_pops() {
        let mut p = Placeholder::new("t", "b");
        assert!(matches!(p.on_event(key(KeyCode::Esc)), Action::Pop));
    }

    #[test]
    fn q_pops() {
        let mut p = Placeholder::new("t", "b");
        assert!(matches!(p.on_event(key(KeyCode::Char('q'))), Action::Pop));
    }

    #[test]
    fn other_keys_do_nothing() {
        let mut p = Placeholder::new("t", "b");
        assert!(matches!(p.on_event(key(KeyCode::Enter)), Action::None));
    }
}
