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

/// Numeric ladder for the `interval_hours` field. `None` sits at the
/// left edge; stepping right advances through `[1, 6, 12, 24, 48]`.
const INTERVAL_LADDER: &[u32] = &[1, 6, 12, 24, 48];

#[derive(Default)]
pub struct UpdateCheckerMenu {
    focus: usize,
}

/// Step the focused row by `delta` (−1 = Left, +1 = Right). Wraps at
/// the ends: stepping past the last interval loops back to `None`, and
/// so does stepping past the last hour. Row 0 (enabled) toggles.
fn step_focused(focus: usize, delta: i32) -> Action {
    Action::MutateSettings(Box::new(move |s| match focus {
        0 => s.update_checker.enabled = !s.update_checker.enabled,
        1 => {
            s.update_checker.interval_hours = step_interval(s.update_checker.interval_hours, delta)
        }
        2 => s.update_checker.daily_at_hour = step_hour(s.update_checker.daily_at_hour, delta),
        _ => {}
    }))
}

fn step_interval(current: Option<u32>, delta: i32) -> Option<u32> {
    // Encoded state: `None` at index 0, then the ladder at 1..=len.
    let cur_idx: i32 = match current {
        None => 0,
        Some(v) => INTERVAL_LADDER
            .iter()
            .position(|&h| h == v)
            .map(|i| i as i32 + 1)
            .unwrap_or(0),
    };
    let len = INTERVAL_LADDER.len() as i32 + 1; // includes the None slot
    let next = ((cur_idx + delta).rem_euclid(len)) as usize;
    if next == 0 {
        None
    } else {
        Some(INTERVAL_LADDER[next - 1])
    }
}

fn step_hour(current: Option<u8>, delta: i32) -> Option<u8> {
    // 25 states: `None` + 0..=23.
    let cur_idx: i32 = match current {
        None => 0,
        Some(h) => i32::from(h) + 1,
    };
    let next = (cur_idx + delta).rem_euclid(25);
    if next == 0 {
        None
    } else {
        Some((next - 1) as u8)
    }
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
            ("←/→", "Step value"),
            ("Enter", "Pick from list"),
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
            KeyCode::Left => step_focused(self.focus, -1),
            KeyCode::Right => step_focused(self.focus, 1),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_interval_right_advances_through_ladder_and_wraps() {
        assert_eq!(step_interval(None, 1), Some(1));
        assert_eq!(step_interval(Some(1), 1), Some(6));
        assert_eq!(step_interval(Some(6), 1), Some(12));
        assert_eq!(step_interval(Some(12), 1), Some(24));
        assert_eq!(step_interval(Some(24), 1), Some(48));
        assert_eq!(step_interval(Some(48), 1), None);
        assert_eq!(step_interval(None, 1), Some(1));
    }

    #[test]
    fn step_interval_left_wraps_backward() {
        assert_eq!(step_interval(None, -1), Some(48));
        assert_eq!(step_interval(Some(1), -1), None);
        assert_eq!(step_interval(Some(6), -1), Some(1));
    }

    #[test]
    fn step_interval_from_off_ladder_value_snaps_to_off_first() {
        // If someone hand-edits their config to `intervalHours: 7`,
        // stepping should bounce out via the "unknown → None" branch.
        assert_eq!(step_interval(Some(7), 1), Some(1));
    }

    #[test]
    fn step_hour_right_advances_with_off_before_zero() {
        assert_eq!(step_hour(None, 1), Some(0));
        assert_eq!(step_hour(Some(0), 1), Some(1));
        assert_eq!(step_hour(Some(22), 1), Some(23));
        assert_eq!(step_hour(Some(23), 1), None);
    }

    #[test]
    fn step_hour_left_wraps() {
        assert_eq!(step_hour(None, -1), Some(23));
        assert_eq!(step_hour(Some(0), -1), None);
        assert_eq!(step_hour(Some(12), -1), Some(11));
    }
}
