//! Interactive smoke-test for every P1 primitive.
//!
//! Run with:
//!
//! ```text
//! cargo run --example dsl_demo -p glassline-tui-dsl
//! ```
//!
//! Key bindings:
//!
//! | Key       | Effect                                                     |
//! |-----------|------------------------------------------------------------|
//! | ↑ / ↓     | Move selection inside the picker list                      |
//! | `/`       | Focus the filter input; typing narrows the list            |
//! | Esc       | Return focus to the list (or dismiss the modal)            |
//! | Enter     | Show a confirmation modal for the highlighted item         |
//! | `t`       | Fire a 2.5-second toast                                    |
//! | `p`       | Toggle the Preview panel (real render pipeline output)     |
//! | `q`       | Quit — terminal is restored via ratatui's panic hook       |
//!
//! The Preview strip renders through the *actual* render pipeline against
//! an empty `Settings`, so what you see there matches what the hot-path
//! `glassline` binary would produce.

use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers};

use glassline_core::{render_context::RenderContext, settings::Settings};
use glassline_tui_dsl::{
    Action, Button, DslApp, DslError, List, Modal, Outcome, Panel, Preview, Screen, TextInput, Ui,
};

fn main() -> Result<(), DslError> {
    let app = DslApp::new(
        Box::new(DemoScreen::new()),
        Settings::default(),
        std::path::PathBuf::from("./demo-scratch.json"),
    );
    let outcome = app.run()?;
    // Print the outcome AFTER ratatui restores the terminal so it
    // lands in the parent shell, not in the alt-screen.
    println!("Demo exited: {outcome:?}");
    Ok(())
}

/// One-screen demo. Owns a `List<&'static str>`, a `TextInput`, a
/// modal-visibility flag, and a preview-visibility flag.
struct DemoScreen {
    list: List<&'static str>,
    filter: TextInput,
    filter_focused: bool,
    show_modal: bool,
    show_preview: bool,
    modal_result: Option<String>,
}

impl DemoScreen {
    fn new() -> Self {
        let mut list = List::new(vec![
            "git-branch",
            "git-changes",
            "git-origin-host",
            "context-percentage",
            "context-bar",
            "tokens-total",
            "session-clock",
            "weekly-usage",
        ]);
        let _ = &mut list; // ownership shuffle for clarity
        Self {
            list,
            filter: TextInput::new().with_hint("filter…").with_max_len(40),
            filter_focused: false,
            show_modal: false,
            show_preview: true,
            modal_result: None,
        }
    }
}

impl Screen for DemoScreen {
    fn title(&self) -> &str {
        "glassline-tui-dsl demo"
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Nav"),
            ("/", "Filter"),
            ("Enter", "Confirm"),
            ("t", "Toast"),
            ("p", "Toggle preview"),
            ("q", "Quit"),
        ]
    }

    fn render(&mut self, ui: &mut Ui) {
        use ratatui::layout::{Constraint, Layout};

        let area = ui.area();
        let [top, middle, filter_row, list_area, footer] = Layout::vertical([
            Constraint::Length(3),                                     // title panel
            Constraint::Length(if self.show_preview { 3 } else { 0 }), // preview panel
            Constraint::Length(3),                                     // filter input
            Constraint::Fill(1),                                       // list
            Constraint::Length(1),                                     // key hints
        ])
        .areas(area);

        // Title panel
        Panel::new(self.title()).render(top, ui.frame, |inner, frame| {
            let hint = if self.filter_focused {
                "  filter focused — type to narrow, Esc to leave"
            } else if self.show_modal {
                "  modal open — Enter/Esc to close"
            } else {
                "  press ↑/↓, /, Enter, t, p, or q"
            };
            frame.render_widget(ratatui::widgets::Paragraph::new(hint), inner);
        });

        // Preview panel (real render pipeline)
        if self.show_preview {
            Panel::new("live preview (real pipeline)").render(middle, ui.frame, |inner, frame| {
                let preview = Preview::new(RenderContext::default, Settings::default);
                preview.render(inner, frame);
            });
        }

        // Filter input
        let filter_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .title("filter");
        self.filter.render(
            filter_row,
            ui.frame,
            Some(filter_block),
            self.filter_focused,
        );

        // List
        Panel::new("widgets").render(list_area, ui.frame, |inner, frame| {
            // Push the current filter into the list before rendering.
            self.list.set_filter(self.filter.value());
            self.list.render(inner, frame, |s| (*s).to_string());
        });

        // Footer keybindings hint
        let hint_line: String = self
            .keybindings()
            .iter()
            .map(|(k, a)| format!("[{k}] {a}"))
            .collect::<Vec<_>>()
            .join("  ");
        ui.render_widget(
            ratatui::widgets::Paragraph::new(hint_line).style(
                ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::DIM),
            ),
            footer,
        );

        // Modal overlay (last so it paints on top)
        if self.show_modal {
            let btns = [Button::new("OK"), Button::new("Cancel")];
            let msg = self.modal_result.as_deref().unwrap_or("Confirm selection?");
            Modal::new("Confirm", msg, &btns)
                .with_size(60, 40)
                .render(area, ui.frame);
        }
    }

    fn on_event(&mut self, ev: Event) -> Action {
        // Modal has focus while open — Esc / Enter dismisses.
        if self.show_modal {
            if let Event::Key(k) = ev {
                if matches!(k.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                    self.show_modal = false;
                }
            }
            return Action::None;
        }

        // Filter has focus while enabled — Esc leaves; other keys forward.
        if self.filter_focused {
            match ev {
                Event::Key(k) if k.code == KeyCode::Esc => {
                    self.filter_focused = false;
                }
                _ => {
                    self.filter.handle_event(&ev);
                }
            }
            return Action::None;
        }

        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Char('q') => Action::Quit { save: false },
            KeyCode::Char('t') => Action::Toast(format!(
                "toast: {}",
                self.list
                    .selected_item(|s| (*s).to_string())
                    .copied()
                    .unwrap_or("(nothing selected)")
            )),
            KeyCode::Char('p') => {
                self.show_preview = !self.show_preview;
                Action::None
            }
            KeyCode::Char('/') => {
                self.filter_focused = true;
                Action::None
            }
            KeyCode::Enter => {
                let sel = self.list.selected_item(|s| (*s).to_string()).copied();
                self.modal_result = sel.map(|s| format!("Selected: {s}"));
                self.show_modal = true;
                Action::None
            }
            KeyCode::Up => {
                self.list.move_up(|s| (*s).to_string());
                Action::None
            }
            KeyCode::Down => {
                self.list.move_down(|s| (*s).to_string());
                Action::None
            }
            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                Action::Quit { save: false }
            }
            _ => Action::None,
        }
    }
}

// Ensure the type unused-import silencer stays inert; only used to
// keep this example compiling if the DSL ever removes `Outcome` from
// its re-exports.
const _: fn() -> Outcome = || Outcome::Discarded;
