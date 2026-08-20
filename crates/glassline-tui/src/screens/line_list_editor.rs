//! `LineListEditor` — pick which line (1/2/3/…) to edit. P3 stub;
//! ItemsEditor plumbing lands in T3.3.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Panel, Screen, Ui};

#[derive(Default)]
pub struct LineListEditor;

impl Screen for LineListEditor {
    fn title(&self) -> &str {
        "Edit Lines"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("Esc", "Back")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        Panel::new("Line list").render(area, ui.frame, |inner, frame| {
            let line_count = ui.settings.lines.len();
            let body = format!(
                "Scratch settings contain {line_count} line(s). Full ItemsEditor lands in T3.3."
            );
            frame.render_widget(Paragraph::new(body), inner);
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
