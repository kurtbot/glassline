//! Basic16 color picker modal. Ansi256 / Truecolor / Gradient modes
//! land in follow-up commits within P3; today Basic16 is the only mode.
//!
//! On Enter, fires the caller's `on_pick` with the chosen color name
//! and pops. Esc pops without picking.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use glassline_tui_dsl::{Action, Screen, Ui, centered_rect};

const COLORS: &[(&str, Color)] = &[
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("cyan", Color::Cyan),
    ("white", Color::Gray),
    ("brightBlack", Color::DarkGray),
    ("brightRed", Color::LightRed),
    ("brightGreen", Color::LightGreen),
    ("brightYellow", Color::LightYellow),
    ("brightBlue", Color::LightBlue),
    ("brightMagenta", Color::LightMagenta),
    ("brightCyan", Color::LightCyan),
    ("brightWhite", Color::White),
];

type OnPick = Box<dyn FnOnce(Option<String>) -> Action>;

pub struct ColorMenu {
    title_text: &'static str,
    selected: usize,
    on_pick: Option<OnPick>,
}

impl ColorMenu {
    pub fn new<F>(title: &'static str, current: Option<&str>, on_pick: F) -> Self
    where
        F: FnOnce(Option<String>) -> Action + 'static,
    {
        let selected = current
            .and_then(|name| COLORS.iter().position(|(n, _)| *n == name))
            .unwrap_or(0);
        Self {
            title_text: title,
            selected,
            on_pick: Some(Box::new(on_pick)),
        }
    }
}

impl Screen for ColorMenu {
    fn title(&self) -> &str {
        self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("↑/↓/←/→", "Nav"), ("Enter", "Pick"), ("Esc", "Cancel")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = centered_rect(ui.area(), 60, 60);
        ui.frame.render_widget(ratatui::widgets::Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title_text);
        let inner = block.inner(area);
        ui.render_widget(block, area);

        let rows = 4usize;
        let cols = 4usize;
        let [grid, hint] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);

        // Render 4×4 grid of color chips.
        let row_areas: [ratatui::layout::Rect; 4] = Layout::vertical([
            Constraint::Ratio(1, rows as u32),
            Constraint::Ratio(1, rows as u32),
            Constraint::Ratio(1, rows as u32),
            Constraint::Ratio(1, rows as u32),
        ])
        .areas(grid);
        for (r, row_area) in row_areas.iter().enumerate() {
            let cols_areas: [ratatui::layout::Rect; 4] = Layout::horizontal([
                Constraint::Ratio(1, cols as u32),
                Constraint::Ratio(1, cols as u32),
                Constraint::Ratio(1, cols as u32),
                Constraint::Ratio(1, cols as u32),
            ])
            .areas(*row_area);
            for (c, col_area) in cols_areas.iter().enumerate() {
                let idx = r * cols + c;
                if idx >= COLORS.len() {
                    continue;
                }
                let (name, color) = COLORS[idx];
                let focused = idx == self.selected;
                let marker = if focused { " > " } else { "   " };
                let style = Style::default().fg(color).add_modifier(if focused {
                    Modifier::BOLD | Modifier::REVERSED
                } else {
                    Modifier::empty()
                });
                let line = Line::from(vec![
                    Span::raw(marker),
                    Span::styled(format!("███ {name}"), style),
                ]);
                ui.frame.render_widget(Paragraph::new(line), *col_area);
            }
        }

        let hint_line = Line::from(vec![Span::styled(
            format!("selected: {}", COLORS[self.selected].0),
            Style::default().add_modifier(Modifier::DIM),
        )]);
        ui.render_widget(Paragraph::new(hint_line), hint);
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        let cols = 4usize;
        let len = COLORS.len();
        match k.code {
            KeyCode::Left => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                Action::None
            }
            KeyCode::Right => {
                if self.selected + 1 < len {
                    self.selected += 1;
                }
                Action::None
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(cols);
                Action::None
            }
            KeyCode::Down => {
                let next = self.selected + cols;
                if next < len {
                    self.selected = next;
                }
                Action::None
            }
            KeyCode::Enter => {
                let pick = Some(COLORS[self.selected].0.to_string());
                let cb = self.on_pick.take().expect("Enter fires once");
                Action::Sequence(vec![Action::Pop, cb(pick)])
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                if let Some(cb) = self.on_pick.take() {
                    Action::Sequence(vec![Action::Pop, cb(None)])
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

    use ratatui::crossterm::event::{Event, KeyEvent, KeyModifiers};

    use super::*;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn initializes_at_named_color_index() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let menu = ColorMenu::new("Color", Some("cyan"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        assert_eq!(menu.selected, 6, "cyan is index 6 in COLORS");
    }

    #[test]
    fn enter_fires_selected_color() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", Some("red"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        menu.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(Some("red".into())));
    }

    #[test]
    fn esc_fires_none() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", Some("red"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        menu.on_event(key(KeyCode::Esc));
        assert_eq!(*sink.lock().unwrap(), Some(None));
    }

    #[test]
    fn arrow_nav_stays_in_bounds() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", Some("black"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        // Up from top row → clamped.
        menu.on_event(key(KeyCode::Up));
        assert_eq!(menu.selected, 0);
        // Right / down.
        menu.on_event(key(KeyCode::Right));
        assert_eq!(menu.selected, 1);
        menu.on_event(key(KeyCode::Down));
        assert_eq!(menu.selected, 5);
    }
}
