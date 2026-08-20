//! `LineListEditor` — pick which line (1/2/3/…) to edit.
//!
//! Renders each line as a row with its widget count + a compact
//! preview of the widget ids. Arrow keys navigate; Enter opens the
//! [`ItemsEditor`] for that line; `n` appends a new (empty) line;
//! `d` removes the highlighted line (with a confirmation modal).

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::settings::WidgetSpec;
use glassline_tui_dsl::{Action, Panel, Preview, Screen, Ui};

use crate::preview_ctx::canned_context;
use crate::screens::confirm_modal::ConfirmModal;
use crate::screens::items_editor::ItemsEditor;

#[derive(Default)]
pub struct LineListEditor {
    cursor: usize,
    last_len: usize,
}

impl Screen for LineListEditor {
    fn title(&self) -> &str {
        "Edit Lines"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Nav"),
            ("Enter", "Edit line"),
            ("n", "New line"),
            ("d", "Delete line"),
            ("Esc", "Back"),
        ]
    }
    fn render(&mut self, ui: &mut Ui) {
        let len = ui.settings.lines.len();
        self.last_len = len;
        if len == 0 {
            self.cursor = 0;
        } else if self.cursor >= len {
            self.cursor = len - 1;
        }

        let area = ui.area();
        let [preview_area, list_area, hint] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        Panel::new("Preview").render(preview_area, ui.frame, |inner, frame| {
            let settings = ui.settings.clone();
            let preview = Preview::new(canned_context, move || settings.clone());
            preview.render(inner, frame);
        });

        Panel::new("Lines").render(list_area, ui.frame, |inner, frame| {
            if ui.settings.lines.is_empty() {
                frame.render_widget(
                    Paragraph::new("(no lines yet)  press `n` to add one")
                        .style(Style::default().add_modifier(Modifier::DIM)),
                    inner,
                );
                return;
            }
            let rows: Vec<Line> = ui
                .settings
                .lines
                .iter()
                .enumerate()
                .map(|(i, widgets)| line_row(i, widgets, i == self.cursor))
                .collect();
            frame.render_widget(Paragraph::new(rows), inner);
        });

        let text = if len == 0 {
            "no lines".to_string()
        } else {
            format!("line {} of {} · Enter to edit", self.cursor + 1, len)
        };
        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                text,
                Style::default().add_modifier(Modifier::DIM),
            )])),
            hint,
        );
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.last_len {
                    self.cursor += 1;
                }
                Action::None
            }
            KeyCode::Char('n') => Action::MutateSettings(Box::new(|s| s.lines.push(Vec::new()))),
            KeyCode::Char('d') => {
                let idx = self.cursor;
                let len = self.last_len;
                if len == 0 {
                    return Action::None;
                }
                Action::Push(Box::new(ConfirmModal::new(
                    "Delete line",
                    format!("Remove line {}? All its widgets go with it.", idx + 1),
                    move |ok| {
                        if !ok {
                            return Action::Pop;
                        }
                        Action::MutateSettings(Box::new(move |s| {
                            if idx < s.lines.len() {
                                s.lines.remove(idx);
                            }
                        }))
                    },
                )))
            }
            KeyCode::Enter => {
                if self.last_len == 0 {
                    return Action::None;
                }
                Action::Push(Box::new(ItemsEditor::new(self.cursor)))
            }
            _ => Action::None,
        }
    }
}

fn line_row<'a>(index: usize, widgets: &'a [WidgetSpec], focused: bool) -> Line<'a> {
    let marker = if focused { "> " } else { "  " };
    let count = format!(" · {} widget(s)", widgets.len());
    let preview: String = widgets
        .iter()
        .take(4)
        .map(|w| w.kind.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    let preview = if widgets.len() > 4 {
        format!("{preview} · …")
    } else {
        preview
    };
    let label = format!("Line {}", index + 1);
    let style = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::from(vec![
        Span::raw(marker),
        Span::styled(label, style),
        Span::styled(count, Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::styled(preview, Style::default().add_modifier(Modifier::DIM)),
    ])
}

#[cfg(test)]
mod tests {
    use glassline_core::settings::Settings;

    use super::*;

    fn apply(action: Action, s: &mut Settings) {
        if let Action::MutateSettings(m) = action {
            m(s);
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(ratatui::crossterm::event::KeyEvent::new(
            code,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
    }

    #[test]
    fn n_appends_a_line() {
        // Settings::default() ships with a non-empty starter layout;
        // the test only cares that pressing `n` grows the line count
        // by one and appends an empty row at the end.
        let mut s = Settings::default();
        let before = s.lines.len();
        let mut ed = LineListEditor::default();
        let action = ed.on_event(key(KeyCode::Char('n')));
        apply(action, &mut s);
        assert_eq!(s.lines.len(), before + 1);
        assert!(s.lines.last().unwrap().is_empty());
    }

    #[test]
    fn arrow_down_advances_within_bounds() {
        let mut ed = LineListEditor {
            cursor: 0,
            last_len: 3,
        };
        ed.on_event(key(KeyCode::Down));
        assert_eq!(ed.cursor, 1);
    }

    #[test]
    fn enter_with_no_lines_is_noop() {
        let mut ed = LineListEditor {
            cursor: 0,
            last_len: 0,
        };
        assert!(matches!(ed.on_event(key(KeyCode::Enter)), Action::None));
    }

    #[test]
    fn enter_with_lines_pushes_items_editor() {
        let mut ed = LineListEditor {
            cursor: 0,
            last_len: 2,
        };
        match ed.on_event(key(KeyCode::Enter)) {
            Action::Push(_) => {}
            other => panic!("expected Push(ItemsEditor), got {other:?}"),
        }
    }
}
