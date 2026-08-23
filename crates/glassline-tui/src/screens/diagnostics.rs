//! `DiagnosticsScreen` — read-only report on where glassline is
//! looking for things, whether the config is healthy, whether the
//! render binary's registry and this editor's `METAS` catalog agree,
//! Claude Code wiring status, and a tail of the debug log.
//!
//! v1.0 keeps this a pure snapshot — no FS-watch, no interactive
//! filtering (design gaps G-iii + v1.1). Reads happen on every render
//! tick so a shell running `glassline install` in parallel is visible
//! next time the user scrolls the log.

use std::path::PathBuf;

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_render::config::default_settings_path;
use glassline_tui_dsl::{Action, Panel, Screen, Ui};
use glassline_widgets::registry::{ALIASES, WIDGETS};

use crate::cli_detect::{self, DetectResult};
use crate::meta::METAS;

const LOG_TAIL_LINES: usize = 50;

pub struct DiagnosticsScreen;

impl Screen for DiagnosticsScreen {
    fn title(&self) -> &str {
        "Diagnostics"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("Esc", "Back")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let [summary_area, log_area] =
            Layout::vertical([Constraint::Length(18), Constraint::Fill(1)]).areas(area);

        Panel::new("Snapshot").render(summary_area, ui.frame, |inner, frame| {
            frame.render_widget(Paragraph::new(summary_lines()), inner);
        });

        Panel::new(&format!("Debug log tail (last {LOG_TAIL_LINES} lines)")).render(
            log_area,
            ui.frame,
            |inner, frame| {
                let lines = log_tail_lines();
                frame.render_widget(Paragraph::new(lines), inner);
            },
        );
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
            _ => Action::None,
        }
    }
}

fn summary_lines() -> Vec<Line<'static>> {
    let mut out = Vec::new();

    // Config resolution
    let config_path = default_settings_path()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unresolved)".into());
    let config_exists = default_settings_path()
        .ok()
        .map(|p| p.exists())
        .unwrap_or(false);
    out.push(kv("Config path", &config_path));
    out.push(kv(
        "Config exists",
        if config_exists {
            "yes"
        } else {
            "no (first-run defaults)"
        },
    ));

    // Widget/META parity
    let widget_count = WIDGETS.entries().count();
    let alias_count = ALIASES.len();
    let canonical_count = widget_count - alias_count;
    let meta_count = METAS.entries().count();
    let parity_ok = canonical_count == meta_count;
    out.push(kv("Registered widgets", &format!("{widget_count} total")));
    out.push(kv(
        "  canonical",
        &format!(
            "{canonical_count}   (with {meta_count} META entries, {} alias{})",
            alias_count,
            if alias_count == 1 { "" } else { "es" }
        ),
    ));
    if !parity_ok {
        out.push(Line::from(vec![Span::styled(
            format!(
                "  ⚠ META drift — {canonical_count} canonical widgets but {meta_count} META entries"
            ),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )]));
    }

    // Environment
    let color_env = std::env::var("COLORTERM").unwrap_or_else(|_| "(unset)".into());
    let term_env = std::env::var("TERM").unwrap_or_else(|_| "(unset)".into());
    let wt = if std::env::var_os("WT_SESSION").is_some() {
        "yes"
    } else {
        "no"
    };
    out.push(kv("$COLORTERM", &color_env));
    out.push(kv("$TERM", &term_env));
    out.push(kv("Windows Terminal", wt));

    // Claude Code wiring
    let (label, note) = claude_code_wiring();
    out.push(kv("Claude Code wiring", &label));
    if let Some(note) = note {
        out.push(Line::from(vec![Span::styled(
            format!("  {note}"),
            Style::default().add_modifier(Modifier::DIM),
        )]));
    }

    // Detected CLIs — snapshot at render time so a shell running
    // `codex plugin enable` in parallel shows up next tick.
    for (candidate, result) in cli_detect::snapshot() {
        let (status, evidence) = match &result {
            DetectResult::Installed { evidence } => (
                "installed".to_string(),
                Some(evidence.display().to_string()),
            ),
            DetectResult::NotInstalled => ("not detected".to_string(), None),
            DetectResult::Unknown => ("(adapter pending)".to_string(), None),
        };
        out.push(kv(
            &format!("Detected: {}", candidate.display_name),
            &status,
        ));
        if let Some(path) = evidence {
            out.push(Line::from(vec![Span::styled(
                format!("  via {path}"),
                Style::default().add_modifier(Modifier::DIM),
            )]));
        }
    }

    // Log path
    let log_path = debug_log_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(unresolved)".into());
    out.push(kv("Debug log", &log_path));

    out
}

fn kv(k: &str, v: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{k:>22}: "),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(v.to_string(), Style::default().add_modifier(Modifier::BOLD)),
    ])
}

/// Read `~/.claude/settings.json` and report whether its `statusLine`
/// entry looks like a glassline install. Returns `(label, note)`.
fn claude_code_wiring() -> (String, Option<String>) {
    let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) else {
        return ("(no HOME)".into(), None);
    };
    let path = PathBuf::from(home).join(".claude").join("settings.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return (
            "not installed".into(),
            Some(format!("no file at {}", path.display())),
        );
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (
            "corrupted".into(),
            Some(format!("bad JSON at {}", path.display())),
        );
    };
    let cmd = json
        .get("statusLine")
        .and_then(|v| v.get("command"))
        .and_then(|v| v.as_str());
    match cmd {
        None => (
            "not installed".into(),
            Some(format!("{} has no statusLine.command", path.display())),
        ),
        Some(c) if c.contains("glassline") => (
            "installed".into(),
            Some(format!("statusLine.command = {c}")),
        ),
        Some(c) => (
            "installed (foreign command)".into(),
            Some(format!("statusLine.command = {c}")),
        ),
    }
}

fn debug_log_path() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(
        PathBuf::from(home)
            .join(".cache")
            .join("glassline")
            .join("debug.log"),
    )
}

fn log_tail_lines() -> Vec<Line<'static>> {
    let Some(path) = debug_log_path() else {
        return vec![dim("(no log path resolved)")];
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return vec![
            dim(&format!("no log file at {}", path.display())),
            dim(""),
            dim("Set GLASSLINE_DEBUG=1 before invoking glassline to enable"),
            dim("event logging (transcript scans, cache hits, usage probes)."),
        ];
    };
    let all: Vec<&str> = raw.lines().collect();
    let start = all.len().saturating_sub(LOG_TAIL_LINES);
    all[start..]
        .iter()
        .map(|s| Line::from((*s).to_string()))
        .collect()
}

fn dim(text: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        text.to_string(),
        Style::default().add_modifier(Modifier::DIM),
    )])
}
