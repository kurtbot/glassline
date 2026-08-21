//! Fixed-option chooser modal. Backing store for `MetaShape::Choice`
//! knobs (widget format variants, powerline themes, flex modes, …).
//!
//! Empty-string options render as `(unset)` and commit as `None`.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear},
};

use glassline_tui_dsl::{Action, List, Screen, Ui, centered_rect};

type OnCommit = Box<dyn FnOnce(Option<String>) -> Action>;

pub struct ChoiceModal {
    title_text: String,
    list: List<&'static str>,
    on_commit: Option<OnCommit>,
}

impl ChoiceModal {
    pub fn new<F>(
        title: impl Into<String>,
        options: &'static [&'static str],
        current: Option<&str>,
        on_commit: F,
    ) -> Self
    where
        F: FnOnce(Option<String>) -> Action + 'static,
    {
        let list = List::new(options.to_vec());
        let mut modal = Self {
            title_text: title.into(),
            list,
            on_commit: Some(Box::new(on_commit)),
        };
        // Position the cursor on the current value if it's in the
        // options list; otherwise leave at 0.
        if let Some(c) = current
            && let Some(pos) = options.iter().position(|opt| *opt == c)
        {
            for _ in 0..pos {
                modal.list.move_down(|s| (*s).to_string());
            }
        }
        modal
    }
}

fn label(s: &&'static str) -> String {
    if s.is_empty() {
        "(unset)".into()
    } else {
        (*s).to_string()
    }
}

impl Screen for ChoiceModal {
    fn title(&self) -> &str {
        &self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("↑/↓", "Nav"), ("Enter", "Pick"), ("Esc", "Cancel")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let rect = centered_rect(ui.frame_area(), 60, 60);
        ui.frame.render_widget(Clear, rect);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title_text.as_str());
        let inner = block.inner(rect);
        ui.render_widget(block, rect);

        let [list_area, _hint]: [Rect; 2] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(inner);
        self.list.render(list_area, ui.frame, label);
        let _ = Modifier::DIM; // silence unused-import if hint removed
        let _ = Style::default;
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                if let Some(cb) = self.on_commit.take() {
                    Action::Sequence(vec![Action::Pop, cb(None)])
                } else {
                    Action::Pop
                }
            }
            KeyCode::Enter => {
                let cb = self.on_commit.take().expect("Enter fires once");
                let choice = self
                    .list
                    .selected_item(label)
                    .copied()
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty());
                Action::Sequence(vec![Action::Pop, cb(choice)])
            }
            KeyCode::Up => {
                self.list.move_up(label);
                Action::None
            }
            KeyCode::Down => {
                self.list.move_down(label);
                Action::None
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

    const OPTS: &[&str] = &["", "50", "80", "90"];

    #[test]
    fn initializes_on_current_value() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let modal = ChoiceModal::new("t", OPTS, Some("80"), {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        assert_eq!(modal.list.selected_item(label).copied(), Some("80"));
    }

    #[test]
    fn enter_commits_selected() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut modal = ChoiceModal::new("t", OPTS, Some("50"), {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        modal.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(Some("50".into())));
    }

    #[test]
    fn enter_on_empty_option_commits_none() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut modal = ChoiceModal::new("t", OPTS, Some(""), {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        modal.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(None));
    }

    #[test]
    fn esc_commits_none() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut modal = ChoiceModal::new("t", OPTS, Some("80"), {
            let sink = sink.clone();
            move |v| {
                *sink.lock().unwrap() = Some(v);
                Action::Pop
            }
        });
        modal.on_event(key(KeyCode::Esc));
        assert_eq!(*sink.lock().unwrap(), Some(None));
    }
}
