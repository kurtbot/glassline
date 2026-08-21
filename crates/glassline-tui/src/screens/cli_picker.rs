//! `CliPickerScreen` — final wizard step. Replaces the earlier
//! `InstallPromptScreen` (single-question "wire into Claude Code?").
//!
//! Renders a multi-select over the CLI detection snapshot plus a
//! scope radio (`--user` / `--project`). Detected CLIs default to
//! selected; not-detected CLIs render informationally (dim, radio-
//! style `( )`, un-toggleable). Enter fires the install; Esc pops.
//!
//! Collapse rule (design v1.0 §4.4): when exactly one CLI is
//! Installed and no others are shown as picker rows, the screen
//! renders as a single-question "Install into <name>? [Y/n]" layout.
//! Same code path fires regardless — the collapse is UI-only.
//!
//! Backend note: P2 still shells out `glassline install --user` for a
//! Claude-only install because `install --for <cli>` (design §4.5)
//! isn't shipped yet. P3 introduces `Action::InstallForCli` and
//! batches over the picker's selection.

use std::process::Command;

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_tui_dsl::{Action, Panel, Screen, Ui};

use crate::cli_detect::{self, CliCandidate, DetectResult};
use crate::screens::info_modal::InfoModal;

/// Which install-scope the picker's Enter fires with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeChoice {
    User,
    Project,
}

impl ScopeChoice {
    fn arg(self) -> &'static str {
        match self {
            Self::User => "--user",
            Self::Project => "--project",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::User => "--user   Install to ~/.claude/settings.json",
            Self::Project => "--project  Install to ./.claude/settings.json here",
        }
    }
}

/// One row in the picker's focus order. CLI rows come first, then
/// two scope-radio rows. Focus wraps around the whole list.
#[derive(Debug, Clone, Copy)]
enum Row {
    /// Index into `snapshot`.
    Cli(usize),
    Scope(ScopeChoice),
}

pub struct CliPickerScreen {
    snapshot: Vec<(&'static CliCandidate, DetectResult)>,
    /// Toggled CLI indices (into `snapshot`). Only detected CLIs are
    /// ever inserted; the not-detected branch of `on_event` refuses.
    toggled: Vec<usize>,
    focus: usize,
    scope: ScopeChoice,
    rows: Vec<Row>,
    /// Cached from construction: does exactly one CLI Installed +
    /// zero Unknown apply? Drives the collapsed one-question render.
    collapsed: bool,
}

impl Default for CliPickerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl CliPickerScreen {
    #[must_use]
    pub fn new() -> Self {
        let snapshot = cli_detect::snapshot();

        // Pre-select every Installed candidate.
        let toggled: Vec<usize> = snapshot
            .iter()
            .enumerate()
            .filter_map(|(i, (_, r))| matches!(r, DetectResult::Installed { .. }).then_some(i))
            .collect();

        // Collapse: exactly one Installed, no Unknown to render.
        let installed_count = snapshot
            .iter()
            .filter(|(_, r)| matches!(r, DetectResult::Installed { .. }))
            .count();
        let unknown_count = snapshot
            .iter()
            .filter(|(_, r)| matches!(r, DetectResult::Unknown))
            .count();
        let collapsed = installed_count == 1 && unknown_count == 0;

        // Focus order: CLI rows first, then scope rows. In collapsed
        // mode we skip the focus scaffolding entirely (Enter/y is the
        // whole interaction) — build rows anyway so tests can inspect.
        let mut rows: Vec<Row> = (0..snapshot.len()).map(Row::Cli).collect();
        rows.push(Row::Scope(ScopeChoice::User));
        rows.push(Row::Scope(ScopeChoice::Project));

        Self {
            snapshot,
            toggled,
            focus: 0,
            scope: ScopeChoice::User,
            rows,
            collapsed,
        }
    }

    /// The selected CLI keys, in snapshot order. Used by the install
    /// action in P3; for now, P2's Enter branch uses this to decide
    /// whether to shell out or short-circuit with a toast.
    #[must_use]
    fn selected_keys(&self) -> Vec<&'static str> {
        self.toggled
            .iter()
            .map(|&i| self.snapshot[i].0.key)
            .collect()
    }

    /// Move focus by `delta`. Wraps around row bounds.
    fn move_focus(&mut self, delta: isize) {
        if self.rows.is_empty() {
            return;
        }
        let n = self.rows.len() as isize;
        self.focus = ((self.focus as isize + delta).rem_euclid(n)) as usize;
    }

    /// Space semantics per design §4.6: toggles a detected CLI row;
    /// switches the scope radio when on a scope row; no-op + toast on
    /// a not-detected CLI row.
    fn handle_space(&mut self) -> Action {
        let Some(row) = self.rows.get(self.focus).copied() else {
            return Action::None;
        };
        match row {
            Row::Cli(idx) => {
                let (candidate, result) = &self.snapshot[idx];
                match result {
                    DetectResult::Installed { .. } => {
                        if let Some(pos) = self.toggled.iter().position(|&i| i == idx) {
                            self.toggled.remove(pos);
                        } else {
                            self.toggled.push(idx);
                            self.toggled.sort_unstable();
                        }
                        Action::None
                    }
                    DetectResult::NotInstalled | DetectResult::Unknown => Action::Toast(format!(
                        "{} is not installed on this machine",
                        candidate.display_name
                    )),
                }
            }
            Row::Scope(choice) => {
                self.scope = choice;
                Action::None
            }
        }
    }

    /// Enter semantics: run install for every selected CLI, push a
    /// summary modal. Zero-selection is a toast + no-op (design §4.2).
    fn handle_enter(&self) -> Action {
        let keys = self.selected_keys();
        if keys.is_empty() {
            return Action::Toast("no CLIs selected — install skipped".into());
        }
        // P2: only Claude is meaningfully installable via the
        // existing single-CLI subcommand. If the selection is
        // exactly ["claude"], shell out; otherwise report the
        // unsupported keys.
        //
        // P3 replaces this branch with `Action::InstallForCli`.
        if keys == ["claude"] {
            let (title, body) = match run_install_user(self.scope) {
                Ok(msg) => ("Install succeeded", msg),
                Err(msg) => ("Install failed", msg),
            };
            return Action::Sequence(vec![
                Action::Pop,
                Action::Push(Box::new(InfoModal::new(title, body))),
            ]);
        }
        // Selection includes non-Claude CLIs but the render binary
        // doesn't ship `install --for <cli>` yet. Explain rather
        // than pretend.
        let extras: Vec<&str> = keys.iter().copied().filter(|k| *k != "claude").collect();
        let body = format!(
            "Selected: {}\n\nOnly Claude Code can be wired up in this build.\n\nAdapter support for {} is tracked in issues #14 / #15 / #16.\n\nProceeding to install into Claude Code only.",
            keys.join(", "),
            extras.join(", "),
        );
        let install_summary = if keys.contains(&"claude") {
            match run_install_user(self.scope) {
                Ok(msg) => format!("\n\n---\n\n{msg}"),
                Err(msg) => format!("\n\n---\n\nInstall failed:\n{msg}"),
            }
        } else {
            "\n\nClaude not selected — nothing to install right now.".into()
        };
        Action::Sequence(vec![
            Action::Pop,
            Action::Push(Box::new(InfoModal::new(
                "Adapter support pending",
                format!("{body}{install_summary}"),
            ))),
        ])
    }
}

impl Screen for CliPickerScreen {
    fn title(&self) -> &str {
        if self.collapsed {
            "Install glassline"
        } else {
            "Pick coding CLIs"
        }
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        if self.collapsed {
            &[("y", "Install"), ("n", "Skip"), ("Esc", "Skip")]
        } else {
            &[
                ("↑/↓", "Focus"),
                ("Space", "Toggle"),
                ("Enter", "Install"),
                ("Esc", "Skip"),
            ]
        }
    }

    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        if self.collapsed {
            self.render_collapsed(area, ui);
        } else {
            self.render_full(area, ui);
        }
    }

    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        if self.collapsed {
            return self.on_event_collapsed(k.code);
        }
        match k.code {
            KeyCode::Up => {
                self.move_focus(-1);
                Action::None
            }
            KeyCode::Down => {
                self.move_focus(1);
                Action::None
            }
            KeyCode::Char(' ') => self.handle_space(),
            KeyCode::Enter => self.handle_enter(),
            KeyCode::Esc | KeyCode::Char('q') => Action::Pop,
            _ => Action::None,
        }
    }
}

impl CliPickerScreen {
    fn render_full(&self, area: ratatui::layout::Rect, ui: &mut Ui) {
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);

        Panel::new("First-run — pick coding CLIs").render(header_area, ui.frame, |inner, frame| {
            frame.render_widget(
                Paragraph::new(Line::from(vec![Span::styled(
                    "  glassline can wire into every CLI it detects on this machine.",
                    Style::default().add_modifier(Modifier::DIM),
                )])),
                inner,
            );
        });

        Panel::new("Actions").render(body_area, ui.frame, |inner, frame| {
            let mut lines: Vec<Line> = Vec::with_capacity(self.rows.len() + 3);
            for (row_idx, row) in self.rows.iter().enumerate() {
                let is_focused = row_idx == self.focus;
                lines.push(self.render_row(*row, is_focused));
                // Blank separator between CLI block and scope block.
                if matches!(row, Row::Cli(i) if *i + 1 == self.snapshot.len()) {
                    lines.push(Line::from(""));
                    lines.push(Line::from(Span::styled(
                        "  Scope:",
                        Style::default().add_modifier(Modifier::DIM),
                    )));
                }
            }
            frame.render_widget(Paragraph::new(lines), inner);
        });
    }

    fn render_row(&self, row: Row, focused: bool) -> Line<'static> {
        let marker = if focused { ">" } else { " " };
        match row {
            Row::Cli(idx) => {
                let (candidate, result) = &self.snapshot[idx];
                let (checkbox, style) = match result {
                    DetectResult::Installed { .. } => {
                        let toggled = self.toggled.contains(&idx);
                        let cb = if toggled { "[x]" } else { "[ ]" };
                        let s = if focused {
                            Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                        } else {
                            Style::default().add_modifier(Modifier::BOLD)
                        };
                        (cb, s)
                    }
                    DetectResult::NotInstalled | DetectResult::Unknown => {
                        ("( )", Style::default().add_modifier(Modifier::DIM))
                    }
                };
                let hint = match result {
                    DetectResult::Installed { .. } => candidate.install_hint.to_string(),
                    DetectResult::NotInstalled => "(not detected)".into(),
                    DetectResult::Unknown => "(adapter pending)".into(),
                };
                Line::from(vec![
                    Span::raw(format!(" {marker} ")),
                    Span::raw(checkbox.to_string()),
                    Span::raw(" "),
                    Span::styled(candidate.display_name.to_string(), style),
                    Span::raw("    "),
                    Span::styled(hint, Style::default().add_modifier(Modifier::DIM)),
                ])
            }
            Row::Scope(choice) => {
                let selected = self.scope == choice;
                let radio = if selected { "(*)" } else { "( )" };
                let style = if focused {
                    Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
                } else if selected {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };
                Line::from(vec![
                    Span::raw(format!(" {marker} ")),
                    Span::raw(radio.to_string()),
                    Span::raw(" "),
                    Span::styled(choice.label().to_string(), style),
                ])
            }
        }
    }

    fn render_collapsed(&self, area: ratatui::layout::Rect, ui: &mut Ui) {
        let Some(installed_idx) = self
            .snapshot
            .iter()
            .position(|(_, r)| matches!(r, DetectResult::Installed { .. }))
        else {
            return;
        };
        let candidate = self.snapshot[installed_idx].0;

        Panel::new("Install").render(area, ui.frame, |inner, frame| {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    format!("  Wire glassline into {}?", candidate.display_name),
                    Style::default().add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from("  Runs `glassline install --user` which edits"),
                Line::from(format!("  {}.", candidate.install_hint)),
                Line::from(""),
                Line::from(Span::styled(
                    "  Press y to install, n to skip. You can always run",
                    Style::default().add_modifier(Modifier::DIM),
                )),
                Line::from(Span::styled(
                    "  `glassline install --user` later from a shell.",
                    Style::default().add_modifier(Modifier::DIM),
                )),
            ];
            frame.render_widget(Paragraph::new(lines), inner);
        });
    }

    fn on_event_collapsed(&self, code: KeyCode) -> Action {
        match code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                Action::Pop
            }
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let (title, body) = match run_install_user(ScopeChoice::User) {
                    Ok(msg) => ("Install succeeded", msg),
                    Err(msg) => ("Install failed", msg),
                };
                Action::Sequence(vec![
                    Action::Pop,
                    Action::Push(Box::new(InfoModal::new(title, body))),
                ])
            }
            _ => Action::None,
        }
    }
}

fn run_install_user(scope: ScopeChoice) -> Result<String, String> {
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
            "Can't find the render binary next to this editor:\n{}",
            render_bin.display()
        ));
    }
    let output = Command::new(&render_bin)
        .args(["install", scope.arg()])
        .output()
        .map_err(|e| format!("spawn `{}`: {e}", render_bin.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "`glassline install {}` failed:\n{stderr}",
            scope.arg()
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(format!(
        "Ran `glassline install {}` — Claude Code will use glassline on next launch.\n\n{}",
        scope.arg(),
        stdout.trim(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pre_selects_installed_indices() {
        // We can't easily fake `snapshot()` here (it consults the real
        // filesystem + PATH), so this test asserts a shape invariant:
        // every entry in `toggled` corresponds to an Installed row.
        let s = CliPickerScreen::new();
        for idx in &s.toggled {
            assert!(
                matches!(s.snapshot[*idx].1, DetectResult::Installed { .. }),
                "toggled row {idx} was not Installed: {:?}",
                s.snapshot[*idx].1
            );
        }
    }

    #[test]
    fn scope_defaults_to_user() {
        let s = CliPickerScreen::new();
        assert_eq!(s.scope, ScopeChoice::User);
    }

    #[test]
    fn focus_starts_at_zero() {
        let s = CliPickerScreen::new();
        assert_eq!(s.focus, 0);
    }

    #[test]
    fn move_focus_wraps() {
        let mut s = CliPickerScreen::new();
        let n = s.rows.len();
        // Moving backward from 0 wraps to last row.
        s.move_focus(-1);
        assert_eq!(s.focus, n - 1);
        // Forward from last wraps to 0.
        s.move_focus(1);
        assert_eq!(s.focus, 0);
    }

    #[test]
    fn zero_selection_enter_returns_toast() {
        let mut s = CliPickerScreen::new();
        s.toggled.clear();
        match s.handle_enter() {
            Action::Toast(text) => {
                assert!(text.contains("no CLIs selected"), "toast text: {text}");
            }
            other => panic!("expected Toast, got {other:?}"),
        }
    }

    #[test]
    fn space_on_scope_row_flips_radio() {
        let mut s = CliPickerScreen::new();
        // Move focus to a scope row. Last two rows are the scope rows.
        s.focus = s.rows.len() - 1; // Project
        let _ = s.handle_space();
        assert_eq!(s.scope, ScopeChoice::Project);
        s.focus = s.rows.len() - 2; // User
        let _ = s.handle_space();
        assert_eq!(s.scope, ScopeChoice::User);
    }

    #[test]
    fn space_on_not_detected_row_toasts() {
        // Find a row that is not `Installed` in the snapshot. Grok's
        // placeholder guarantees at least one such row exists.
        let mut s = CliPickerScreen::new();
        let Some(non_installed_idx) = s
            .snapshot
            .iter()
            .position(|(_, r)| !matches!(r, DetectResult::Installed { .. }))
        else {
            // If every CLI is installed, this test can't run — the
            // CI runner would need Codex + Grok binaries which we
            // deliberately don't ship. Bail cleanly.
            return;
        };
        s.focus = non_installed_idx;
        let toggled_before = s.toggled.clone();
        match s.handle_space() {
            Action::Toast(text) => {
                assert!(text.contains("not installed"), "toast text: {text}");
            }
            other => panic!("expected Toast, got {other:?}"),
        }
        // Toggled set unchanged.
        assert_eq!(s.toggled, toggled_before);
    }

    #[test]
    fn collapsed_flag_matches_snapshot_shape() {
        let s = CliPickerScreen::new();
        let installed = s
            .snapshot
            .iter()
            .filter(|(_, r)| matches!(r, DetectResult::Installed { .. }))
            .count();
        let unknown = s
            .snapshot
            .iter()
            .filter(|(_, r)| matches!(r, DetectResult::Unknown))
            .count();
        assert_eq!(s.collapsed, installed == 1 && unknown == 0);
    }

    #[test]
    fn row_count_matches_snapshot_plus_two_scope_rows() {
        let s = CliPickerScreen::new();
        assert_eq!(s.rows.len(), s.snapshot.len() + 2);
    }
}
