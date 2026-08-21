//! `ImportExportMenu` — one-shot migration from ccstatusline + export
//! of the current scratch settings to a chosen path. Wraps the CLI's
//! `run_import` engine so the diff modal (T4.7 gap G-vi) shares its
//! source of truth with the `glassline import` subcommand.

use std::path::PathBuf;

use ratatui::{
    crossterm::event::{Event, KeyCode},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::settings::Settings;
use glassline_render::import::{ImportOpts, run_import};
use glassline_tui_dsl::{Action, Panel, Screen, Ui};

use crate::screens::text_edit_modal::TextEditModal;

#[derive(Default)]
pub struct ImportExportMenu {
    focus: usize,
}

const ROWS: &[(&str, &str)] = &[
    (
        "Import from ccstatusline (auto-detect)",
        "Load your ccstatusline settings into scratch. Press `s` on Main Menu to persist.",
    ),
    (
        "Import from file (choose path)",
        "Load any glassline / ccstatusline settings.json into scratch (migrates on the fly).",
    ),
    (
        "Export current settings to file",
        "Write the current scratch to a chosen JSON path.",
    ),
];

impl Screen for ImportExportMenu {
    fn title(&self) -> &str {
        "Import / Export"
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[("↑/↓", "Focus"), ("Enter", "Run"), ("Esc", "Back")]
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        Panel::new("Import / Export").render(area, ui.frame, |inner, frame| {
            let lines: Vec<Line> = ROWS
                .iter()
                .enumerate()
                .map(|(i, (label, hint))| {
                    let marker = if i == self.focus { "> " } else { "  " };
                    let title_style = if i == self.focus {
                        Style::default().add_modifier(Modifier::REVERSED)
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
            let mut all: Vec<Line> = lines;
            all.push(Line::from(""));
            all.push(Line::from(vec![Span::styled(
                "Outcome shows as a toast in the bottom-right corner.",
                Style::default().add_modifier(Modifier::DIM),
            )]));
            frame.render_widget(Paragraph::new(all), inner);
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
                0 => self.import_from_ccstatusline(),
                1 => self.push_import_from_file_prompt(),
                2 => self.push_export_prompt(),
                _ => Action::None,
            },
            _ => Action::None,
        }
    }
}

impl ImportExportMenu {
    fn import_from_ccstatusline(&mut self) -> Action {
        // dry_run=true: don't touch disk; we only want the migrated JSON
        // so we can splice it into scratch.
        let opts = ImportOpts {
            dry_run: true,
            ..ImportOpts::default()
        };
        match run_import(&opts) {
            Err(e) => Action::Toast(format!("Import failed: {e}")),
            Ok(report) => {
                let target_json = report.target_json.clone();
                let summary = format!(
                    "Imported from {} (v{} → v{}) — {} lines, {} built-ins, {} ext",
                    report.source.display(),
                    report.source_version,
                    report.target_version,
                    report.lines,
                    report.widgets_builtin,
                    report.widgets_external,
                );
                match serde_json::from_str::<Settings>(&target_json) {
                    Ok(new_settings) => Action::Sequence(vec![
                        Action::MutateSettings(Box::new(move |s| *s = new_settings)),
                        Action::Toast(summary),
                    ]),
                    Err(e) => Action::Toast(format!("Parse migrated JSON failed: {e}")),
                }
            }
        }
    }

    fn push_import_from_file_prompt(&self) -> Action {
        Action::Push(Box::new(TextEditModal::new(
            "import path",
            "path to a settings.json (glassline or ccstatusline)",
            None,
            500,
            |v| {
                let Some(path_str) = v else {
                    return Action::Toast("Import cancelled".into());
                };
                let path = PathBuf::from(path_str.trim());
                if path.as_os_str().is_empty() {
                    return Action::Toast("Empty path — import cancelled".into());
                }
                if !path.exists() {
                    return Action::Toast(format!("Not found: {}", path.display()));
                }
                let opts = ImportOpts {
                    from: Some(path.clone()),
                    dry_run: true,
                    ..ImportOpts::default()
                };
                match run_import(&opts) {
                    Err(e) => Action::Toast(format!("Import failed: {e}")),
                    Ok(report) => {
                        let target_json = report.target_json.clone();
                        let summary = format!(
                            "Imported from {} (v{} → v{}) — {} lines, {} built-ins, {} ext",
                            report.source.display(),
                            report.source_version,
                            report.target_version,
                            report.lines,
                            report.widgets_builtin,
                            report.widgets_external,
                        );
                        match serde_json::from_str::<Settings>(&target_json) {
                            Ok(new_settings) => Action::Sequence(vec![
                                Action::MutateSettings(Box::new(move |s| *s = new_settings)),
                                Action::Toast(summary),
                            ]),
                            Err(e) => Action::Toast(format!("Parse migrated JSON failed: {e}")),
                        }
                    }
                }
            },
        )))
    }

    fn push_export_prompt(&self) -> Action {
        Action::Push(Box::new(TextEditModal::new(
            "export path",
            "file OR folder — folders get my-glassline-export.json auto-appended",
            None,
            500,
            |v| {
                let Some(path_str) = v else {
                    return Action::Toast("Export cancelled".into());
                };
                let raw = path_str.trim();
                if raw.is_empty() {
                    return Action::Toast("Empty path — export cancelled".into());
                }
                let path = resolve_export_path(raw);
                // Read-only access to scratch — export just serializes
                // it and writes; no mutation. Toast reports outcome so
                // stderr doesn't leak into the alt-screen.
                Action::WithSettings(Box::new(move |settings| {
                    match export_settings(&path, settings) {
                        Ok(()) => Action::Toast(format!("Exported to {}", path.display())),
                        Err(err) => Action::Toast(format!("Export failed: {err}")),
                    }
                }))
            },
        )))
    }
}

const DEFAULT_EXPORT_FILENAME: &str = "my-glassline-export.json";

/// If the user gave us a directory (existing or trailing-slash),
/// append the default filename so the write lands *inside* it rather
/// than trying to overwrite the directory itself.
fn resolve_export_path(raw: &str) -> PathBuf {
    let looks_like_dir = raw.ends_with('/') || raw.ends_with('\\');
    let path = PathBuf::from(raw);
    if looks_like_dir || path.is_dir() {
        return path.join(DEFAULT_EXPORT_FILENAME);
    }
    // If the path exists but is a file, keep it as-is (user is
    // choosing to overwrite). Otherwise: honour the exact name they
    // typed — they probably know what they want.
    path
}

fn export_settings(path: &std::path::Path, settings: &Settings) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| format!("serialize: {e}"))?;
    std::fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_export_path_appends_default_when_trailing_slash() {
        let p = resolve_export_path("C:/Users/kurt/");
        assert_eq!(p.file_name().unwrap(), DEFAULT_EXPORT_FILENAME);
    }

    #[test]
    fn resolve_export_path_appends_default_when_trailing_backslash() {
        let p = resolve_export_path(r"C:\Users\kurt\");
        assert_eq!(p.file_name().unwrap(), DEFAULT_EXPORT_FILENAME);
    }

    #[test]
    fn resolve_export_path_keeps_file_path_verbatim() {
        let p = resolve_export_path("C:/Users/kurt/foo.json");
        assert_eq!(p.file_name().unwrap(), "foo.json");
    }

    #[test]
    fn resolve_export_path_bare_name_untouched() {
        // No slash and doesn't exist — keep as-is; write will land in cwd.
        let p = resolve_export_path("my-export.json");
        assert_eq!(p.file_name().unwrap(), "my-export.json");
    }
}
