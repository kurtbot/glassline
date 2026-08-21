//! `UpdateCheckerMenu` — enable + pick cadence.
//!
//! Cadence has two orthogonal knobs: `interval_hours` (every N hours)
//! and `daily_at_hour` (once per day at HH:00 local). Either or both
//! may be set; the render binary's future update-check implementation
//! fires on whichever comes first.
//!
//! Scope note: the actual periodic check isn't implemented in the
//! render binary yet — this screen just persists user preference into
//! the settings.json schema.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::settings::Settings;
use glassline_tui_dsl::{Action, Panel, Screen, Ui};

use crate::screens::choice_modal::ChoiceModal;

const INTERVAL_OPTIONS: &[&str] = &["", "1", "6", "12", "24", "48"];
const HOUR_OPTIONS: &[&str] = &[
    "", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16",
    "17", "18", "19", "20", "21", "22", "23",
];

const ROWS: usize = 3;

#[derive(Default)]
pub struct UpdateCheckerMenu {
    focus: usize,
}

fn row_values(s: &Settings) -> [(String, String); ROWS] {
    let uc = &s.update_checker;
    let interval = uc
        .interval_hours
        .map(|h| format!("every {h}h"))
        .unwrap_or_else(|| "(off)".into());
    let daily = uc
        .daily_at_hour
        .map(|h| format!("{h:02}:00 local"))
        .unwrap_or_else(|| "(off)".into());
    [
        (
            "enabled".into(),
            if uc.enabled { "on" } else { "off" }.into(),
        ),
        ("interval".into(), interval),
        ("daily at".into(), daily),
    ]
}

impl Screen for UpdateCheckerMenu {
    fn title(&self) -> &str {
        "Update Checker"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Focus"),
            ("Enter", "Edit"),
            ("Space", "Toggle enabled"),
            ("Esc", "Back"),
        ]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let [body, hint] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(area);
        let rows = row_values(ui.settings);
        Panel::new("Update checker").render(body, ui.frame, |inner, frame| {
            let lines: Vec<Line> = rows
                .iter()
                .enumerate()
                .map(|(i, (k, v))| {
                    let marker = if i == self.focus { "> " } else { "  " };
                    let style = if i == self.focus {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    Line::from(vec![
                        Span::raw(marker),
                        Span::styled(format!("{k} = "), style),
                        Span::styled(v.clone(), Style::default().add_modifier(Modifier::BOLD)),
                    ])
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), inner);
        });
        ui.render_widget(
            Paragraph::new(vec![
                Line::from(vec![Span::styled(
                    "Note: the periodic check is not implemented in the render binary yet — these",
                    Style::default().add_modifier(Modifier::DIM),
                )]),
                Line::from(vec![Span::styled(
                    "settings are stored for when it lands. Both cadence knobs are optional.",
                    Style::default().add_modifier(Modifier::DIM),
                )]),
            ]),
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
                if self.focus > 0 {
                    self.focus -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.focus + 1 < ROWS {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Char(' ') => Action::MutateSettings(Box::new(|s| {
                s.update_checker.enabled = !s.update_checker.enabled;
            })),
            KeyCode::Enter => match self.focus {
                0 => Action::MutateSettings(Box::new(|s| {
                    s.update_checker.enabled = !s.update_checker.enabled;
                })),
                1 => Action::Push(Box::new(ChoiceModal::new(
                    "Check every N hours",
                    INTERVAL_OPTIONS,
                    None,
                    |pick| {
                        Action::MutateSettings(Box::new(move |s| {
                            s.update_checker.interval_hours =
                                pick.and_then(|v| v.parse::<u32>().ok());
                        }))
                    },
                ))),
                2 => Action::Push(Box::new(ChoiceModal::new(
                    "Also check daily at HH:00 local",
                    HOUR_OPTIONS,
                    None,
                    |pick| {
                        Action::MutateSettings(Box::new(move |s| {
                            s.update_checker.daily_at_hour =
                                pick.and_then(|v| v.parse::<u8>().ok());
                        }))
                    },
                ))),
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}
