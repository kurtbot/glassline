//! First-run wizard — a linear flow shown when
//! `glassline-tui` launches with no existing settings.json. Four steps:
//!
//! 1. **Welcome** — intro + Continue/Skip.
//! 2. **Template pick** — choose a starter layout (minimal / powerline /
//!    dev / power-user). Live preview updates as you nav.
//! 3. **Color level** — Basic16 / Ansi256 / Truecolor, seeded by the
//!    `COLORTERM` / `TERM` env vars.
//! 4. **CLI picker** — detects which coding CLIs are on the machine
//!    (Claude Code, Codex, Grok…) and lets the user multi-select
//!    which to wire glassline into. Single-detected-CLI machines
//!    collapse to the current single-question layout.
//!
//! Each step `Replace`s itself with the next so `Esc` cleanly bails
//! the whole wizard without a back-stack. On the last step, the
//! wizard pops out to the `MainMenu` push that sits underneath it.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::{
    color::ColorLevel,
    settings::{FlexMode, Settings, WidgetSpec},
};
use glassline_tui_dsl::{Action, Panel, Preview, Screen, Ui};

use crate::preview_ctx::canned_context;
use crate::screens::main_menu::preview_height;

/// Top-level entry — call from `main` when `LoadOutcome::FirstRun`
/// fires. Returns the screen that should sit on top of MainMenu.
#[must_use]
pub fn wizard_entry() -> Box<dyn Screen> {
    Box::new(WelcomeScreen)
}

// ---------------------------------------------------------------------------
// Step 1 — Welcome
// ---------------------------------------------------------------------------

struct WelcomeScreen;

impl Screen for WelcomeScreen {
    fn title(&self) -> &str {
        "Welcome to glassline"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("Enter", "Continue"), ("Esc", "Skip wizard")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        Panel::new("Welcome").render(area, ui.frame, |inner, frame| {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "  glassline — a fast Rust-native Claude Code status line.",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("  This first-run wizard will help you:"),
                Line::from("    • pick a starter layout"),
                Line::from("    • confirm your terminal's color level"),
                Line::from("    • wire glassline into Claude Code"),
                Line::from(""),
                Line::from(Span::styled(
                    "  Enter to continue, Esc to skip and edit from Main Menu.",
                    Style::default().add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        });
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Enter => Action::Replace(Box::new(TemplatePickScreen::default())),
            _ => Action::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Step 2 — Template pick
// ---------------------------------------------------------------------------

#[derive(Default)]
struct TemplatePickScreen {
    focus: usize,
}

type Template = (&'static str, &'static str, fn() -> Settings);
const TEMPLATES: &[Template] = &[
    (
        "Minimal",
        "cwd  |  git-branch  |  context %",
        template_minimal,
    ),
    (
        "Dev",
        "model / git / changes  //  tokens + speed + compaction",
        template_dev,
    ),
    (
        "Power user",
        "3 lines — everything on: context bar, usage, speed, cost, session",
        template_power_user,
    ),
];

impl Screen for TemplatePickScreen {
    fn title(&self) -> &str {
        "Pick a starter layout"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Focus"),
            ("Enter", "Use this layout"),
            ("Esc", "Skip"),
        ]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let ph = preview_height(TEMPLATES[self.focus].2().lines.len());
        let [preview_area, list_area] =
            Layout::vertical([Constraint::Length(ph), Constraint::Fill(1)]).areas(area);

        // Preview renders the FOCUSED template's settings (not the
        // app scratch), so the user can see what they'll get before
        // committing.
        Panel::new("Preview (focused template)").render(preview_area, ui.frame, |inner, frame| {
            let build = TEMPLATES[self.focus].2;
            let preview = Preview::new(canned_context, build);
            preview.render(inner, frame);
        });

        Panel::new("Templates").render(list_area, ui.frame, |inner, frame| {
            let lines: Vec<Line> = TEMPLATES
                .iter()
                .enumerate()
                .map(|(i, (name, hint, _))| {
                    let marker = if i == self.focus { "> " } else { "  " };
                    let title_style = if i == self.focus {
                        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::BOLD)
                    };
                    Line::from(vec![
                        Span::raw(marker),
                        Span::styled((*name).to_string(), title_style),
                        Span::styled(
                            format!("  {hint}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ),
                    ])
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), inner);
        });
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Up => {
                if self.focus > 0 {
                    self.focus -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.focus + 1 < TEMPLATES.len() {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Enter => {
                let build = TEMPLATES[self.focus].2;
                let template = build();
                Action::Sequence(vec![
                    Action::MutateSettings(Box::new(move |s| *s = template)),
                    Action::Replace(Box::new(ColorLevelScreen::default())),
                ])
            }
            _ => Action::None,
        }
    }
}

pub fn template_minimal() -> Settings {
    Settings {
        lines: vec![vec![
            WidgetSpec::new("m1", "current-working-dir").with_color("blue"),
            WidgetSpec::new("m2", "separator"),
            WidgetSpec::new("m3", "git-branch").with_color("magenta"),
            WidgetSpec::new("m4", "separator"),
            WidgetSpec::new("m5", "context-percentage").with_color("yellow"),
        ]],
        ..Settings::in_memory_defaults()
    }
}

pub fn template_dev() -> Settings {
    Settings {
        lines: vec![
            vec![
                WidgetSpec::new("d1", "model").with_color("cyan"),
                WidgetSpec::new("d2", "separator"),
                WidgetSpec::new("d3", "git-branch").with_color("magenta"),
                WidgetSpec::new("d4", "separator"),
                WidgetSpec::new("d5", "git-changes").with_color("brightGreen"),
            ],
            vec![
                WidgetSpec::new("d6", "context-percentage").with_color("yellow"),
                WidgetSpec::new("d7", "separator"),
                WidgetSpec::new("d8", "tokens-total").with_color("brightYellow"),
                WidgetSpec::new("d9", "separator"),
                WidgetSpec::new("d10", "total-speed").with_color("cyan"),
                WidgetSpec::new("d11", "separator"),
                WidgetSpec::new("d12", "compaction-counter").with_color("brightBlack"),
            ],
        ],
        ..Settings::in_memory_defaults()
    }
}

pub fn template_power_user() -> Settings {
    Settings {
        lines: vec![
            vec![
                WidgetSpec::new("p1", "model").with_color("cyan"),
                WidgetSpec::new("p2", "separator"),
                WidgetSpec::new("p3", "context-bar").with_color("green"),
                WidgetSpec::new("p4", "separator"),
                WidgetSpec::new("p5", "git-branch").with_color("magenta"),
                WidgetSpec::new("p6", "separator"),
                WidgetSpec::new("p7", "git-changes").with_color("brightGreen"),
                WidgetSpec::new("p8", "separator"),
                WidgetSpec::new("p9", "current-working-dir").with_color("blue"),
            ],
            vec![
                WidgetSpec::new("p10", "context-percentage").with_color("yellow"),
                WidgetSpec::new("p11", "separator"),
                WidgetSpec::new("p12", "session-clock").with_color("yellow"),
                WidgetSpec::new("p13", "separator"),
                WidgetSpec::new("p14", "weekly-reset-timer").with_color("brightBlue"),
                WidgetSpec::new("p15", "separator"),
                WidgetSpec::new("p16", "input-speed").with_color("cyan"),
                WidgetSpec::new("p17", "separator"),
                WidgetSpec::new("p18", "output-speed").with_color("cyan"),
            ],
            vec![
                WidgetSpec::new("p19", "session-usage").with_color("brightGreen"),
                WidgetSpec::new("p20", "separator"),
                WidgetSpec::new("p21", "weekly-usage").with_color("brightCyan"),
                WidgetSpec::new("p22", "separator"),
                WidgetSpec::new("p23", "thinking-effort").with_color("magenta"),
                WidgetSpec::new("p24", "separator"),
                WidgetSpec::new("p25", "session-cost").with_color("brightYellow"),
            ],
        ],
        flex_mode: FlexMode::FullMinus40,
        ..Settings::in_memory_defaults()
    }
}

// ---------------------------------------------------------------------------
// Step 3 — Color level
// ---------------------------------------------------------------------------

#[derive(Default)]
struct ColorLevelScreen {
    focus: usize,
}

const COLOR_LEVELS: &[(&str, ColorLevel)] = &[
    ("None (mono)", ColorLevel::None),
    ("Basic 16", ColorLevel::Ansi16),
    ("Ansi 256", ColorLevel::Ansi256),
    ("Truecolor (24-bit)", ColorLevel::Truecolor),
];

fn detect_color_level() -> ColorLevel {
    if let Ok(v) = std::env::var("COLORTERM")
        && (v.contains("truecolor") || v.contains("24bit"))
    {
        return ColorLevel::Truecolor;
    }
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("256") {
            return ColorLevel::Ansi256;
        }
        if term.contains("color") {
            return ColorLevel::Ansi16;
        }
    }
    // Windows Terminal / modern terminals default to truecolor even
    // without setting COLORTERM.
    if std::env::var_os("WT_SESSION").is_some() {
        return ColorLevel::Truecolor;
    }
    ColorLevel::Ansi256
}

impl ColorLevelScreen {
    fn new_with_detected() -> Self {
        let detected = detect_color_level();
        let focus = COLOR_LEVELS
            .iter()
            .position(|(_, lv)| *lv as u8 == detected as u8)
            .unwrap_or(2);
        Self { focus }
    }
}

impl Screen for ColorLevelScreen {
    fn title(&self) -> &str {
        "Color level"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Pick"),
            ("Enter", "Confirm"),
            ("Esc", "Skip wizard"),
        ]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let detected = detect_color_level();
        let detected_name = COLOR_LEVELS
            .iter()
            .find(|(_, lv)| *lv as u8 == detected as u8)
            .map(|(n, _)| *n)
            .unwrap_or("(unknown)");
        Panel::new("Color level").render(area, ui.frame, |inner, frame| {
            let mut lines = vec![
                Line::from(""),
                Line::from(vec![Span::styled(
                    format!("  Detected from environment: {detected_name}"),
                    Style::default().add_modifier(Modifier::DIM),
                )]),
                Line::from(""),
            ];
            for (i, (name, _)) in COLOR_LEVELS.iter().enumerate() {
                let marker = if i == self.focus { "> " } else { "  " };
                let style = if i == self.focus {
                    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(vec![
                    Span::raw(marker),
                    Span::styled((*name).to_string(), style),
                ]));
            }
            frame.render_widget(Paragraph::new(lines), inner);
        });
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Up => {
                if self.focus > 0 {
                    self.focus -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.focus + 1 < COLOR_LEVELS.len() {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Enter => {
                let level = COLOR_LEVELS[self.focus].1;
                Action::Sequence(vec![
                    Action::MutateSettings(Box::new(move |s| s.color_level = level)),
                    Action::Replace(Box::new(crate::screens::CliPickerScreen::new())),
                ])
            }
            _ => Action::None,
        }
    }
}

// Override the Default impl above so the wizard uses the env-seeded
// starting focus. Wrapper keeps the Default derive shape simple.
impl TemplatePickScreen {
    #[must_use]
    #[allow(dead_code)]
    fn with_focus(focus: usize) -> Self {
        Self { focus }
    }
}

impl From<()> for ColorLevelScreen {
    fn from(_: ()) -> Self {
        ColorLevelScreen::new_with_detected()
    }
}

// Step 4 (CLI picker) lives in `screens/cli_picker.rs` — replaced the
// old single-question InstallPromptScreen so the wizard can detect
// multiple coding CLIs and let the user pick which to wire glassline
// into.
