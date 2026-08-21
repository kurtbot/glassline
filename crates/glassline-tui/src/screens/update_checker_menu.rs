//! `UpdateCheckerMenu` — single bool toggle. Space or Enter flips it.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Panel, Screen, Ui};

#[derive(Default)]
pub struct UpdateCheckerMenu;

impl Screen for UpdateCheckerMenu {
    fn title(&self) -> &str {
        "Update Checker"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("Space / Enter", "Toggle"), ("Esc", "Back")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let enabled = ui.settings.update_checker.enabled;
        Panel::new("Update checker").render(area, ui.frame, |inner, frame| {
            let line = Line::from(vec![
                Span::styled("  enabled = ", Style::default().add_modifier(Modifier::DIM)),
                Span::styled(
                    if enabled { "on" } else { "off" },
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]);
            frame.render_widget(Paragraph::new(line), inner);
        });
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
            KeyCode::Enter | KeyCode::Char(' ') => Action::MutateSettings(Box::new(|s| {
                s.update_checker.enabled = !s.update_checker.enabled;
            })),
            _ => Action::None,
        }
    }
}
