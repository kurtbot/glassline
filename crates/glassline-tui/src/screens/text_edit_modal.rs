//! Single-line text-input modal. Reused by every knob type that needs
//! a text buffer: `MetaShape::Text`, `MetaShape::Integer` (numeric
//! validation is caller-provided), Value knobs. Numeric constraints
//! are enforced by the caller in its `on_commit` closure.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

use glassline_tui_dsl::{Action, Screen, TextInput, Ui, centered_rect};

type OnCommit = Box<dyn FnOnce(Option<String>) -> Action>;

pub struct TextEditModal {
    title_text: String,
    hint: String,
    input: TextInput,
    on_commit: Option<OnCommit>,
}

impl TextEditModal {
    pub fn new<F>(
        title: impl Into<String>,
        hint: impl Into<String>,
        initial: Option<&str>,
        max_len: usize,
        on_commit: F,
    ) -> Self
    where
        F: FnOnce(Option<String>) -> Action + 'static,
    {
        let hint_s: String = hint.into();
        let mut input = TextInput::new()
            .with_hint(hint_s.clone())
            .with_max_len(max_len);
        if let Some(v) = initial {
            input = input.with_value(v);
        }
        Self {
            title_text: title.into(),
            hint: hint_s,
            input,
            on_commit: Some(Box::new(on_commit)),
        }
    }
}

impl Screen for TextEditModal {
    fn title(&self) -> &str {
        &self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("type", "Edit"),
            ("Enter", "Save"),
            ("Ctrl-U", "Clear"),
            ("Esc", "Cancel"),
        ]
    }
    fn render(&mut self, ui: &mut Ui) {
        let rect = centered_rect(ui.frame_area(), 60, 30);
        ui.frame.render_widget(Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title_text.as_str());
        let inner = block.inner(rect);
        ui.render_widget(block, rect);

        let [hint_row, input_row, _spacer]: [Rect; 3] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Fill(1),
        ])
        .areas(inner);

        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                self.hint.as_str(),
                Style::default().add_modifier(Modifier::DIM),
            )])),
            hint_row,
        );

        let input_block = Block::default().borders(Borders::ALL).title("value");
        self.input
            .render(input_row, ui.frame, Some(input_block), true);
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc => {
                if let Some(cb) = self.on_commit.take() {
                    Action::Sequence(vec![Action::Pop, cb(None)])
                } else {
                    Action::Pop
                }
            }
            KeyCode::Enter => {
                let value = self.input.value().to_string();
                let out = if value.is_empty() { None } else { Some(value) };
                let cb = self.on_commit.take().expect("Enter fires once");
                Action::Sequence(vec![Action::Pop, cb(out)])
            }
            KeyCode::Char('u')
                if k.modifiers
                    .contains(ratatui::crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.input.clear();
                Action::None
            }
            _ => {
                self.input.handle_event(&Event::Key(k));
                Action::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ratatui::crossterm::event::{Event, KeyEvent, KeyModifiers};

    use super::*;

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }
    fn special(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn enter_commits_typed_value() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut modal = TextEditModal::new("title", "hint", None, 50, {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        modal.on_event(key('h'));
        modal.on_event(key('i'));
        modal.on_event(special(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(Some("hi".into())));
    }

    #[test]
    fn enter_on_empty_commits_none() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut modal = TextEditModal::new("title", "hint", None, 50, {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        modal.on_event(special(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(None));
    }

    #[test]
    fn esc_commits_none() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut modal = TextEditModal::new("title", "hint", Some("preset"), 50, {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        modal.on_event(special(KeyCode::Esc));
        assert_eq!(*sink.lock().unwrap(), Some(None));
    }
}
