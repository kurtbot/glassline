//! `MainMenu` — the editor's top-level screen. A vertical list of
//! commands (Edit Lines, Powerline, Global Defaults, …) with a live
//! preview strip pinned at the top so users see what their scratch
//! settings render as before diving into a subscreen.
//!
//! In P3 only the "Edit Lines" entry is functional; the rest push
//! placeholder screens that render "coming in P4" and pop on Esc. That
//! keeps the shell navigable while later phases fill in behaviour.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, List, Panel, Preview, Screen, Ui};

use crate::preview_ctx::canned_context;
use crate::screens::line_list_editor::LineListEditor;
use crate::screens::placeholder::Placeholder;

/// One command in the main menu. Static — the array below is the
/// single source of truth.
#[derive(Debug, Clone, Copy)]
struct MenuEntry {
    label: &'static str,
    hint: &'static str,
    action: MenuAction,
}

#[derive(Debug, Clone, Copy)]
enum MenuAction {
    EditLines,
    Powerline,
    GlobalDefaults,
    TerminalOptions,
    UpdateChecker,
    ImportExport,
    InstallUninstall,
    Diagnostics,
    Save,
    Quit,
}

const ENTRIES: &[MenuEntry] = &[
    MenuEntry {
        label: "Edit Lines",
        hint: "Add / remove / reorder widgets on each line.",
        action: MenuAction::EditLines,
    },
    MenuEntry {
        label: "Powerline",
        hint: "Separator glyphs, theme, invert-background, auto-align.",
        action: MenuAction::Powerline,
    },
    MenuEntry {
        label: "Global Defaults",
        hint: "Colors, separator character, padding, git cache TTL.",
        action: MenuAction::GlobalDefaults,
    },
    MenuEntry {
        label: "Terminal Options",
        hint: "Flex mode, compact threshold, width override.",
        action: MenuAction::TerminalOptions,
    },
    MenuEntry {
        label: "Update Checker",
        hint: "Enable/disable + cadence for glassline update notifications.",
        action: MenuAction::UpdateChecker,
    },
    MenuEntry {
        label: "Import / Export",
        hint: "Migrate from ccstatusline or export current settings.",
        action: MenuAction::ImportExport,
    },
    MenuEntry {
        label: "Install / Uninstall",
        hint: "Wire glassline into ~/.claude/settings.json (or remove it).",
        action: MenuAction::InstallUninstall,
    },
    MenuEntry {
        label: "Diagnostics",
        hint: "Config resolution, log tail, widget/META parity.",
        action: MenuAction::Diagnostics,
    },
    MenuEntry {
        label: "Save",
        hint: "Atomic write of the scratch settings to disk.",
        action: MenuAction::Save,
    },
    MenuEntry {
        label: "Quit",
        hint: "Exit the editor (prompts if there are unsaved changes).",
        action: MenuAction::Quit,
    },
];

fn entry_label(e: &&'static MenuEntry) -> String {
    e.label.to_string()
}

/// Height (rows, including borders) the preview panel should reserve
/// for a settings config with `line_count` lines. Clamps to
/// `[3, 8]` — enough to always show at least one row, capped so the
/// preview doesn't swallow the screen when someone stacks many lines.
pub(crate) fn preview_height(line_count: usize) -> u16 {
    let content = line_count.clamp(1, 6);
    (content as u16) + 2
}

/// The main menu screen.
pub struct MainMenu {
    list: List<&'static MenuEntry>,
}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MainMenu {
    #[must_use]
    pub fn new() -> Self {
        Self {
            list: List::new(ENTRIES.iter().collect()),
        }
    }
}

impl Screen for MainMenu {
    fn title(&self) -> &str {
        "glassline — main menu"
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Nav"),
            ("Enter", "Open"),
            ("s", "Save"),
            ("q", "Quit"),
        ]
    }

    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let preview_h = preview_height(ui.settings.lines.len());
        let [preview_area, menu_area, hint_row] = Layout::vertical([
            Constraint::Length(preview_h),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        // Live preview at the top.
        Panel::new("Preview").render(preview_area, ui.frame, |inner, frame| {
            let preview = Preview::new(canned_context, {
                let settings = ui.settings.clone();
                move || settings.clone()
            });
            preview.render(inner, frame);
        });

        // Menu list.
        Panel::new("Menu").render(menu_area, ui.frame, |inner, frame| {
            self.list.render(inner, frame, entry_label);
        });

        // Hint row — description of the highlighted entry.
        let hint = self
            .list
            .selected_item(entry_label)
            .copied()
            .map_or("", |e| e.hint);
        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            )])),
            hint_row,
        );
    }

    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Up => {
                self.list.move_up(entry_label);
                Action::None
            }
            KeyCode::Down => {
                self.list.move_down(entry_label);
                Action::None
            }
            KeyCode::Char('q') => Action::Quit { save: false },
            KeyCode::Char('s') => Action::Save,
            KeyCode::Enter => {
                let Some(entry) = self.list.selected_item(entry_label).copied() else {
                    return Action::None;
                };
                dispatch(entry.action)
            }
            _ => Action::None,
        }
    }
}

fn dispatch(action: MenuAction) -> Action {
    match action {
        MenuAction::EditLines => Action::Push(Box::new(LineListEditor::default())),
        MenuAction::Save => Action::Save,
        MenuAction::Quit => Action::Quit { save: false },
        MenuAction::Powerline => Action::Push(Box::new(crate::screens::PowerlineSetup::default())),
        MenuAction::GlobalDefaults => Action::Push(Box::new(Placeholder::new(
            "Global Defaults",
            "Global default overrides land in P4.",
        ))),
        MenuAction::TerminalOptions => {
            Action::Push(Box::new(crate::screens::TerminalOptionsMenu::default()))
        }
        MenuAction::UpdateChecker => {
            Action::Push(Box::new(crate::screens::UpdateCheckerMenu::default()))
        }
        MenuAction::ImportExport => {
            Action::Push(Box::new(crate::screens::ImportExportMenu::default()))
        }
        MenuAction::InstallUninstall => Action::Push(Box::new(Placeholder::new(
            "Install / Uninstall",
            "Claude Code wiring lands in P5.",
        ))),
        MenuAction::Diagnostics => Action::Push(Box::new(Placeholder::new(
            "Diagnostics",
            "Config resolution + log tail land in P5.",
        ))),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }
    fn special(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn opens_with_first_entry_highlighted() {
        let menu = MainMenu::new();
        let selected = menu.list.selected_item(entry_label).copied();
        assert_eq!(selected.map(|e| e.label), Some("Edit Lines"));
    }

    #[test]
    fn arrow_down_advances_selection() {
        let mut menu = MainMenu::new();
        let _ = menu.on_event(special(KeyCode::Down));
        assert_eq!(
            menu.list
                .selected_item(entry_label)
                .copied()
                .map(|e| e.label),
            Some("Powerline")
        );
    }

    #[test]
    fn quit_returns_quit_action_no_save() {
        let mut menu = MainMenu::new();
        assert!(matches!(
            menu.on_event(key('q')),
            Action::Quit { save: false }
        ));
    }

    #[test]
    fn save_shortcut_returns_save_action() {
        let mut menu = MainMenu::new();
        assert!(matches!(menu.on_event(key('s')), Action::Save));
    }

    #[test]
    fn enter_on_edit_lines_pushes_screen() {
        let mut menu = MainMenu::new();
        // Default selection = "Edit Lines".
        match menu.on_event(special(KeyCode::Enter)) {
            Action::Push(_) => {}
            other => panic!("expected Push, got {other:?}"),
        }
    }
}
