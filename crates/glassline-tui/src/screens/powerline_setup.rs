//! `PowerlineSetup` — enable/disable + the primary separator glyph +
//! theme name + auto-align + continue-across-lines. v1.0 covers the
//! knobs 90% of users actually touch; the multi-separator editor,
//! per-index invert-background matrix, and start/end caps land in
//! v1.1 (design §7 gaps G-iv).

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
use crate::screens::text_edit_modal::TextEditModal;

/// Curated Nerd Font powerline separator glyphs. First one is the
/// canonical right-facing chevron (`U+E0B0`), which ratatui + most
/// terminals with a Nerd Font render out of the box.
const SEPARATOR_GLYPHS: &[&str] = &[
    "\u{E0B0}", //
    "\u{E0B4}", //
    "\u{E0B8}", //
    "\u{E0BC}", //
    "\u{E0C0}", //
    "\u{E0C4}", //
    ">",        // ascii fallback
    "|",        // ascii pipe
];

#[derive(Default)]
pub struct PowerlineSetup {
    focus: usize,
}

const ROWS: usize = 5;

fn row_values(s: &Settings) -> [(String, String); ROWS] {
    let pw = &s.powerline;
    let sep_display = pw
        .separators
        .first()
        .map(|s| {
            format!(
                "{:?}  (U+{:04X})",
                s,
                s.chars().next().map_or(0u32, |c| c as u32)
            )
        })
        .unwrap_or_else(|| "(none)".into());
    let theme = pw.theme.as_deref().unwrap_or("(default)").to_string();
    [
        (
            "enabled".into(),
            if pw.enabled { "on" } else { "off" }.into(),
        ),
        ("primary separator".into(), sep_display),
        ("theme".into(), theme),
        (
            "auto-align".into(),
            if pw.auto_align { "on" } else { "off" }.into(),
        ),
        (
            "continue theme across lines".into(),
            if pw.continue_theme_across_lines {
                "on"
            } else {
                "off"
            }
            .into(),
        ),
    ]
}

impl Screen for PowerlineSetup {
    fn title(&self) -> &str {
        "Powerline"
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
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);
        let rows = row_values(ui.settings);
        Panel::new("Powerline").render(body, ui.frame, |inner, frame| {
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
                "Multi-separator + invert-bg matrix + caps land in v1.1 — hand-edit for those.",
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
                if self.focus + 1 < ROWS {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Char(' ') => match self.focus {
                0 => {
                    Action::MutateSettings(Box::new(|s| s.powerline.enabled = !s.powerline.enabled))
                }
                3 => Action::MutateSettings(Box::new(|s| {
                    s.powerline.auto_align = !s.powerline.auto_align;
                })),
                4 => Action::MutateSettings(Box::new(|s| {
                    s.powerline.continue_theme_across_lines =
                        !s.powerline.continue_theme_across_lines;
                })),
                _ => Action::None,
            },
            KeyCode::Enter => match self.focus {
                0 => {
                    Action::MutateSettings(Box::new(|s| s.powerline.enabled = !s.powerline.enabled))
                }
                1 => Action::Push(Box::new(ChoiceModal::new(
                    "Primary separator glyph",
                    SEPARATOR_GLYPHS,
                    None,
                    |pick| {
                        let Some(glyph) = pick else {
                            return Action::None;
                        };
                        Action::MutateSettings(Box::new(move |s| {
                            if s.powerline.separators.is_empty() {
                                s.powerline.separators.push(glyph);
                                s.powerline.separator_invert_background.push(false);
                            } else {
                                s.powerline.separators[0] = glyph;
                            }
                        }))
                    },
                ))),
                2 => Action::Push(Box::new(TextEditModal::new(
                    "theme name",
                    "e.g. \"forest\" or \"vaporwave\" — blank clears",
                    None,
                    60,
                    |v| Action::MutateSettings(Box::new(move |s| s.powerline.theme = v)),
                ))),
                3 => Action::MutateSettings(Box::new(|s| {
                    s.powerline.auto_align = !s.powerline.auto_align;
                })),
                4 => Action::MutateSettings(Box::new(|s| {
                    s.powerline.continue_theme_across_lines =
                        !s.powerline.continue_theme_across_lines;
                })),
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}
