//! `TerminalOptionsMenu` — flex-mode + compact-threshold + git-cache
//! TTL + minimalist-mode. Small settings surface; renders as a list of
//! rows with Enter opening the appropriate editor sub-modal.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::settings::{FlexMode, Settings};
use glassline_tui_dsl::{Action, Panel, Screen, Ui};

use crate::screens::choice_modal::ChoiceModal;
use crate::screens::text_edit_modal::TextEditModal;

#[derive(Default)]
pub struct TerminalOptionsMenu {
    focus: usize,
}

impl TerminalOptionsMenu {
    const ROWS: usize = 4;
}

const FLEX_MODES: &[&str] = &["full", "full-minus-40", "full-until-compact"];

const FLEX_LADDER: &[FlexMode] = &[
    FlexMode::Full,
    FlexMode::FullMinus40,
    FlexMode::FullUntilCompact,
];

/// Sensible bounds for the two integer knobs.
const COMPACT_MIN: u32 = 20;
const COMPACT_MAX: u32 = 200;
const COMPACT_STEP: u32 = 5;
const GIT_TTL_MIN: u32 = 0;
const GIT_TTL_MAX: u32 = 3_600;
const GIT_TTL_STEP: u32 = 1;

fn flex_mode_as_str(m: FlexMode) -> &'static str {
    match m {
        FlexMode::Full => "full",
        FlexMode::FullMinus40 => "full-minus-40",
        FlexMode::FullUntilCompact => "full-until-compact",
    }
}

fn parse_flex_mode(s: &str) -> Option<FlexMode> {
    Some(match s {
        "full" => FlexMode::Full,
        "full-minus-40" => FlexMode::FullMinus40,
        "full-until-compact" => FlexMode::FullUntilCompact,
        _ => return None,
    })
}

/// Step the focused row by `delta` (−1 = Left, +1 = Right).
///
/// - Row 0 (flex mode): cycle through the ladder, wrapping.
/// - Row 1 (compact threshold): ±5 cols, clamped to `[20, 200]`.
/// - Row 2 (git cache TTL): ±1 s, clamped to `[0, 3600]`.
/// - Row 3 (minimalist): toggle.
fn step_focused(focus: usize, delta: i32) -> Action {
    Action::MutateSettings(Box::new(move |s| match focus {
        0 => s.flex_mode = step_flex(s.flex_mode, delta),
        1 => {
            s.compact_threshold = step_clamped(
                s.compact_threshold,
                delta,
                COMPACT_STEP,
                COMPACT_MIN,
                COMPACT_MAX,
            )
        }
        2 => {
            s.git_cache_ttl_seconds = step_clamped(
                s.git_cache_ttl_seconds,
                delta,
                GIT_TTL_STEP,
                GIT_TTL_MIN,
                GIT_TTL_MAX,
            )
        }
        3 => s.minimalist_mode = !s.minimalist_mode,
        _ => {}
    }))
}

fn step_flex(current: FlexMode, delta: i32) -> FlexMode {
    let cur_idx = FLEX_LADDER.iter().position(|m| *m == current).unwrap_or(0) as i32;
    let len = FLEX_LADDER.len() as i32;
    let next = (cur_idx + delta).rem_euclid(len) as usize;
    FLEX_LADDER[next]
}

fn step_clamped(current: u32, delta: i32, step: u32, min: u32, max: u32) -> u32 {
    let signed = i64::from(current) + i64::from(delta) * i64::from(step);
    signed.clamp(i64::from(min), i64::from(max)) as u32
}

fn rows(settings: &Settings) -> Vec<(String, String)> {
    vec![
        (
            "flex mode".into(),
            flex_mode_as_str(settings.flex_mode).into(),
        ),
        (
            "compact threshold (cols)".into(),
            settings.compact_threshold.to_string(),
        ),
        (
            "git cache TTL (seconds)".into(),
            settings.git_cache_ttl_seconds.to_string(),
        ),
        (
            "minimalist mode".into(),
            if settings.minimalist_mode {
                "on"
            } else {
                "off"
            }
            .into(),
        ),
    ]
}

impl Screen for TerminalOptionsMenu {
    fn title(&self) -> &str {
        "Terminal Options"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Focus"),
            ("←/→", "Step value"),
            ("Enter", "Pick from list"),
            ("Space", "Toggle bool"),
            ("Esc", "Back"),
        ]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let [body, hint] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
        let rows = rows(ui.settings);
        Panel::new("Terminal options").render(body, ui.frame, |inner, frame| {
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
            Paragraph::new(Line::from(vec![Span::styled(
                "←/→ steps values inline · Enter opens the full list · Esc goes back",
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
                if self.focus > 0 {
                    self.focus -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.focus + 1 < Self::ROWS {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Left => step_focused(self.focus, -1),
            KeyCode::Right => step_focused(self.focus, 1),
            KeyCode::Char(' ') => {
                if self.focus == 3 {
                    Action::MutateSettings(Box::new(|s| {
                        s.minimalist_mode = !s.minimalist_mode;
                    }))
                } else {
                    Action::None
                }
            }
            KeyCode::Enter => match self.focus {
                0 => Action::Push(Box::new(ChoiceModal::new(
                    "Flex mode",
                    FLEX_MODES,
                    None,
                    |pick| {
                        let Some(name) = pick else {
                            return Action::None;
                        };
                        let Some(mode) = parse_flex_mode(&name) else {
                            return Action::Toast(format!("unknown flex mode: {name}"));
                        };
                        Action::MutateSettings(Box::new(move |s| s.flex_mode = mode))
                    },
                ))),
                1 => Action::Push(Box::new(TextEditModal::new(
                    "compact threshold",
                    "integer number of columns (e.g. 60)",
                    None,
                    6,
                    |v| {
                        let Some(text) = v else {
                            return Action::None;
                        };
                        match text.parse::<u32>() {
                            Ok(n) => Action::MutateSettings(Box::new(move |s| {
                                s.compact_threshold = n;
                            })),
                            Err(_) => Action::Toast(format!("\"{text}\" is not an integer")),
                        }
                    },
                ))),
                2 => Action::Push(Box::new(TextEditModal::new(
                    "git cache TTL (s)",
                    "integer seconds (e.g. 5)",
                    None,
                    6,
                    |v| {
                        let Some(text) = v else {
                            return Action::None;
                        };
                        match text.parse::<u32>() {
                            Ok(n) => Action::MutateSettings(Box::new(move |s| {
                                s.git_cache_ttl_seconds = n;
                            })),
                            Err(_) => Action::Toast(format!("\"{text}\" is not an integer")),
                        }
                    },
                ))),
                3 => Action::MutateSettings(Box::new(|s| {
                    s.minimalist_mode = !s.minimalist_mode;
                })),
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
    fn step_flex_cycles_forward_and_wraps() {
        assert_eq!(step_flex(FlexMode::Full, 1), FlexMode::FullMinus40);
        assert_eq!(
            step_flex(FlexMode::FullMinus40, 1),
            FlexMode::FullUntilCompact
        );
        assert_eq!(step_flex(FlexMode::FullUntilCompact, 1), FlexMode::Full);
    }

    #[test]
    fn step_flex_cycles_backward_and_wraps() {
        assert_eq!(step_flex(FlexMode::Full, -1), FlexMode::FullUntilCompact);
        assert_eq!(
            step_flex(FlexMode::FullUntilCompact, -1),
            FlexMode::FullMinus40
        );
    }

    #[test]
    fn step_clamped_advances_by_step() {
        assert_eq!(step_clamped(60, 1, 5, 20, 200), 65);
        assert_eq!(step_clamped(60, -1, 5, 20, 200), 55);
    }

    #[test]
    fn step_clamped_saturates_at_edges() {
        assert_eq!(step_clamped(200, 1, 5, 20, 200), 200);
        assert_eq!(step_clamped(20, -1, 5, 20, 200), 20);
    }

    #[test]
    fn step_clamped_handles_out_of_band_current() {
        // If a user hand-edits their config outside the band, we still
        // step from where they were, then clamp back into range.
        assert_eq!(step_clamped(500, -1, 5, 20, 200), 200);
        assert_eq!(step_clamped(5, 1, 5, 20, 200), 20);
    }
}
