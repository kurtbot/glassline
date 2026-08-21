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
            ("Enter", "Edit"),
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
                "Enter to edit · Space toggles minimalist · Esc to go back",
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
