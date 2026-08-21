//! `InstallMenu` — shell out to `glassline install` / `glassline
//! uninstall`. Complements the first-run wizard's install prompt; this
//! is the "change my mind later" entry point.
//!
//! Reads `~/.claude/settings.json` on every render to keep the status
//! line accurate, so running `glassline install` from a shell shows up
//! next tick without needing to reopen the screen.

use std::{path::PathBuf, process::Command};

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Panel, Screen, Ui};

use crate::screens::confirm_modal::ConfirmModal;
use crate::screens::info_modal::InfoModal;

#[derive(Default)]
pub struct InstallMenu {
    focus: usize,
}

/// One-shot rows — labels only. Enter dispatches per index.
const ROWS: &[(&str, &str)] = &[
    (
        "Install (--user)",
        "Wire glassline into ~/.claude/settings.json",
    ),
    (
        "Install (--project)",
        "Wire glassline into ./.claude/settings.json in the current directory",
    ),
    (
        "Uninstall (--user)",
        "Remove the statusLine entry from ~/.claude/settings.json (asks first)",
    ),
];

impl Screen for InstallMenu {
    fn title(&self) -> &str {
        "Install / Uninstall"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("↑/↓", "Focus"), ("Enter", "Run"), ("Esc", "Back")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let [status, body] =
            Layout::vertical([Constraint::Length(4), Constraint::Fill(1)]).areas(area);

        Panel::new("Current status").render(status, ui.frame, |inner, frame| {
            let (label, note, ok) = probe_wiring();
            let label_style = if ok {
                Style::default()
                    .fg(Color::LightGreen)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD)
            };
            let lines = vec![
                Line::from(vec![
                    Span::styled(
                        "  Claude Code wiring: ",
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                    Span::styled(label, label_style),
                ]),
                Line::from(vec![Span::styled(
                    format!("  {note}"),
                    Style::default().add_modifier(Modifier::DIM),
                )]),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        });

        Panel::new("Actions").render(body, ui.frame, |inner, frame| {
            let lines: Vec<Line> = ROWS
                .iter()
                .enumerate()
                .map(|(i, (label, hint))| {
                    let marker = if i == self.focus { "> " } else { "  " };
                    let title_style = if i == self.focus {
                        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                    } else {
                        Style::default().add_modifier(Modifier::BOLD)
                    };
                    Line::from(vec![
                        Span::raw(marker),
                        Span::styled((*label).to_string(), title_style),
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
            KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
            KeyCode::Up => {
                if self.focus > 0 {
                    self.focus -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.focus + 1 < ROWS.len() {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Enter => match self.focus {
                0 => run_and_modal(&["install", "--user"]),
                1 => run_and_modal(&["install", "--project"]),
                2 => Action::Push(Box::new(ConfirmModal::new(
                    "Uninstall glassline",
                    "Remove the statusLine entry from ~/.claude/settings.json?",
                    |ok| {
                        if !ok {
                            return Action::None;
                        }
                        run_and_modal(&["uninstall", "--user"])
                    },
                ))),
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}

/// Probe `~/.claude/settings.json` to describe wiring state. Returns
/// `(label, note, is_glassline)`. Reused from `diagnostics` in spirit
/// but duplicated here so this screen has zero cross-screen coupling.
fn probe_wiring() -> (&'static str, String, bool) {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        return ("(no HOME)", String::new(), false);
    };
    let path = PathBuf::from(home).join(".claude").join("settings.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (
            "not installed",
            format!("no file at {}", path.display()),
            false,
        );
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (
            "corrupted",
            format!("bad JSON at {}", path.display()),
            false,
        );
    };
    let cmd = json
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str());
    match cmd {
        None => (
            "not installed",
            format!("{} has no statusLine.command", path.display()),
            false,
        ),
        Some(c) if c.contains("glassline") => {
            ("installed", format!("statusLine.command = {c}"), true)
        }
        Some(c) => (
            "installed (foreign)",
            format!("statusLine.command = {c}"),
            false,
        ),
    }
}

/// Spawn the sibling `glassline` render binary with `args` and open
/// an `InfoModal` reporting stdout/stderr. Non-blocking — modal Pops
/// on Enter/Esc.
fn run_and_modal(args: &[&str]) -> Action {
    let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let (title, body) = match run_render_binary(&owned) {
        Ok(msg) => ("Success", msg),
        Err(msg) => ("Failed", msg),
    };
    Action::Push(Box::new(InfoModal::new(title, body)))
}

fn run_render_binary(args: &[String]) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("resolve current_exe: {e}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| "no parent dir for current_exe".to_string())?;
    let render_bin = dir.join(if cfg!(windows) {
        "glassline.exe"
    } else {
        "glassline"
    });
    if !render_bin.exists() {
        return Err(format!(
            "Render binary not found next to the editor:\n{}",
            render_bin.display()
        ));
    }
    let output = Command::new(&render_bin)
        .args(args)
        .output()
        .map_err(|e| format!("spawn `{} {}`: {e}", render_bin.display(), args.join(" ")))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "`glassline {}` exited {}:\n\n{}",
            args.join(" "),
            output.status,
            if stderr.is_empty() {
                stdout.to_string()
            } else {
                stderr.to_string()
            }
        ));
    }
    Ok(format!(
        "Ran `glassline {}`:\n\n{}",
        args.join(" "),
        stdout.trim()
    ))
}
