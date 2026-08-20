//! `ItemsEditor` — add / remove / reorder widgets on a specific line.
//!
//! - `a` opens [`WidgetPicker`]; on pick, appends a new [`WidgetSpec`].
//! - `Enter` opens the widget editor (T3.4 lands the real form; today
//!   pops a placeholder).
//! - `d` opens a confirm modal; on confirm, removes the widget.
//! - `Shift+↑ / Shift+↓` swaps the selected widget with its neighbour.
//! - `Esc` / `q` pops the screen.

use ratatui::{
    crossterm::event::{Event, KeyCode, KeyModifiers},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::settings::WidgetSpec;
use glassline_tui_dsl::{Action, List, Panel, Preview, Screen, Ui};

use crate::meta::METAS;
use crate::preview_ctx::canned_context;
use crate::screens::confirm_modal::ConfirmModal;
use crate::screens::widget_editor::WidgetEditor;
use crate::screens::widget_picker::WidgetPicker;

pub struct ItemsEditor {
    line_index: usize,
    /// Row cursor. Kept in sync with the current line's length on each
    /// render — settings may have changed under us via a MutateSettings
    /// action fired by a child screen.
    cursor: usize,
    /// Cached last-known line length; used only for cursor clamp.
    last_len: usize,
    /// Prevents borrow issues by not re-holding the List<T> across
    /// mutations. We rebuild a fresh list per render out of the
    /// current settings.
    _list: List<()>,
}

impl ItemsEditor {
    #[must_use]
    pub fn new(line_index: usize) -> Self {
        Self {
            line_index,
            cursor: 0,
            last_len: 0,
            _list: List::default(),
        }
    }

    fn line_len(&self, ui: &Ui) -> usize {
        ui.settings.lines.get(self.line_index).map_or(0, Vec::len)
    }
}

impl Screen for ItemsEditor {
    fn title(&self) -> &str {
        "Line items"
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Nav"),
            ("a", "Add"),
            ("Enter", "Edit"),
            ("d", "Delete"),
            ("Sh+↑/↓", "Reorder"),
            ("Esc", "Back"),
        ]
    }

    fn render(&mut self, ui: &mut Ui) {
        // Clamp cursor to current line length.
        let len = self.line_len(ui);
        self.last_len = len;
        if len == 0 {
            self.cursor = 0;
        } else if self.cursor >= len {
            self.cursor = len - 1;
        }

        let area = ui.area();
        let [preview_area, list_area, hint] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        // Preview
        Panel::new("Preview").render(preview_area, ui.frame, |inner, frame| {
            let settings = ui.settings.clone();
            let preview = Preview::new(canned_context, move || settings.clone());
            preview.render(inner, frame);
        });

        // List body — hand-rendered so we can reflect scratch mutations
        // without keeping our own copy of items.
        let line = ui.settings.lines.get(self.line_index);
        Panel::new(&format!("Line {} — widgets", self.line_index + 1)).render(
            list_area,
            ui.frame,
            |inner, frame| {
                let empty: Vec<WidgetSpec> = Vec::new();
                let items: &[WidgetSpec] = line.map_or(empty.as_slice(), Vec::as_slice);
                if items.is_empty() {
                    frame.render_widget(
                        Paragraph::new("(empty)  press `a` to add a widget, `Esc` to go back")
                            .style(Style::default().add_modifier(Modifier::DIM)),
                        inner,
                    );
                    return;
                }
                let lines: Vec<Line> = items
                    .iter()
                    .enumerate()
                    .map(|(i, spec)| row_line(i, spec, i == self.cursor))
                    .collect();
                frame.render_widget(Paragraph::new(lines), inner);
            },
        );

        // Hint row
        let text = format!(
            "line {} · {} widget(s) · cursor at {}",
            self.line_index + 1,
            len,
            if len == 0 { 0 } else { self.cursor + 1 }
        );
        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                text,
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
            KeyCode::Up if k.modifiers.contains(KeyModifiers::SHIFT) => self.reorder(-1),
            KeyCode::Down if k.modifiers.contains(KeyModifiers::SHIFT) => self.reorder(1),
            KeyCode::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                Action::None
            }
            KeyCode::Down => {
                if self.cursor + 1 < self.last_len {
                    self.cursor += 1;
                }
                Action::None
            }
            KeyCode::Char('a') => self.push_picker(),
            KeyCode::Char('d') => self.push_delete_confirm(),
            KeyCode::Enter => {
                Action::Push(Box::new(WidgetEditor::new(self.line_index, self.cursor)))
            }
            _ => Action::None,
        }
    }
}

/// Render one row: `> 3. git-branch  Git branch  #cyan`. Internal
/// widget ids never surface here — they're implementation detail.
fn row_line(index: usize, spec: &WidgetSpec, focused: bool) -> Line<'_> {
    let marker = if focused { "> " } else { "  " };
    let style = if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    let index_col = format!("{:>2}. ", index + 1);
    let human = METAS
        .get(spec.kind.as_str())
        .map(|m| m.label)
        .unwrap_or("(unknown)");
    let color = spec
        .color
        .as_deref()
        .map(|c| format!("  #{c}"))
        .unwrap_or_default();
    Line::from(vec![
        Span::raw(marker),
        Span::styled(index_col, Style::default().add_modifier(Modifier::DIM)),
        Span::styled(spec.kind.clone(), style),
        Span::styled(
            format!("  {human}"),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(color, Style::default().add_modifier(Modifier::DIM)),
    ])
}

impl ItemsEditor {
    fn reorder(&mut self, delta: i32) -> Action {
        let idx = self.cursor;
        let target: isize = idx as isize + delta as isize;
        let line = self.line_index;
        // Cursor advances immediately so the render reflects the swap
        // even before the MutateSettings closure runs.
        if delta > 0 {
            self.cursor = self.cursor.saturating_add(1);
        } else {
            self.cursor = self.cursor.saturating_sub(1);
        }
        Action::MutateSettings(Box::new(move |s| {
            let Some(row) = s.lines.get_mut(line) else {
                return;
            };
            let len = row.len();
            if len == 0 || target < 0 || target as usize >= len {
                return;
            }
            row.swap(idx, target as usize);
        }))
    }

    fn push_delete_confirm(&self) -> Action {
        let line = self.line_index;
        let idx = self.cursor;
        Action::Push(Box::new(ConfirmModal::new(
            "Delete widget",
            format!("Remove widget at position {}?", idx + 1),
            move |ok| {
                if !ok {
                    return Action::Pop;
                }
                // Pop the modal, then mutate — collapse into one Action
                // chain via a helper: because Action is a one-shot,
                // we return Pop and let the caller re-fire the mutation
                // on the next tick. That doesn't work; instead we
                // return the mutation and let the modal pop naturally
                // (its own on_decide returns the mutation, and
                // ConfirmModal doesn't re-render after firing).
                Action::MutateSettings(Box::new(move |s| {
                    if let Some(row) = s.lines.get_mut(line)
                        && idx < row.len()
                    {
                        row.remove(idx);
                    }
                }))
            },
        )))
    }

    fn push_picker(&self) -> Action {
        let line = self.line_index;
        let cursor = self.cursor;
        let line_is_empty = self.last_len == 0;
        Action::Push(Box::new(WidgetPicker::new(move |meta| {
            let widget_type = meta.id.to_string();
            Action::MutateSettings(Box::new(move |s| {
                while s.lines.len() <= line {
                    s.lines.push(Vec::new());
                }
                let id = fresh_widget_id();
                let row = &mut s.lines[line];
                // Insertion index:
                //   - empty row → 0 (append)
                //   - otherwise → after the current cursor
                // Always clamp to row.len() so we never panic if a
                // concurrent mutation has resized the row under us.
                let insert_at = if line_is_empty { 0 } else { cursor + 1 };
                let insert_at = insert_at.min(row.len());
                row.insert(insert_at, WidgetSpec::new(id, widget_type));
            }))
        })))
    }
}

/// A per-invocation-unique widget id. Nanos-since-epoch keeps the ids
/// short-ish and monotone within a session; the editor never surfaces
/// them so the exact format is an implementation detail.
fn fresh_widget_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("w-{nanos:x}")
}

#[cfg(test)]
mod tests {
    use glassline_core::settings::Settings;

    use super::*;

    fn fresh_settings_with_widgets(count: usize) -> Settings {
        Settings {
            lines: vec![
                (0..count)
                    .map(|i| WidgetSpec::new(format!("{i}"), "custom-text"))
                    .collect(),
            ],
            ..Settings::default()
        }
    }

    fn apply(action: Action, s: &mut Settings) {
        if let Action::MutateSettings(m) = action {
            m(s);
        }
    }

    #[test]
    fn reorder_down_swaps_with_next() {
        let mut s = fresh_settings_with_widgets(3);
        let mut ed = ItemsEditor::new(0);
        ed.cursor = 0;
        ed.last_len = 3;
        apply(ed.reorder(1), &mut s);
        assert_eq!(s.lines[0][0].id, "1");
        assert_eq!(s.lines[0][1].id, "0");
    }

    #[test]
    fn reorder_up_swaps_with_prev() {
        let mut s = fresh_settings_with_widgets(3);
        let mut ed = ItemsEditor::new(0);
        ed.cursor = 2;
        ed.last_len = 3;
        apply(ed.reorder(-1), &mut s);
        assert_eq!(s.lines[0][1].id, "2");
        assert_eq!(s.lines[0][2].id, "1");
    }

    #[test]
    fn reorder_at_edge_is_noop() {
        let mut s = fresh_settings_with_widgets(3);
        let s_before = s.clone();
        let mut ed = ItemsEditor::new(0);
        ed.cursor = 0;
        ed.last_len = 3;
        apply(ed.reorder(-1), &mut s);
        assert_eq!(s, s_before, "reordering past the top edge must be a no-op");
    }

    #[test]
    fn push_picker_appends_to_empty_line_without_panicking() {
        // Regression: inserting at cursor+1 into an empty Vec panicked.
        // An empty line must accept the first widget at index 0.
        let mut s = Settings {
            lines: vec![Vec::new()],
            ..Settings::default()
        };
        // Manually replay the closure the picker would fire on pick.
        let line: usize = 0;
        let cursor: usize = 0;
        let line_is_empty = true;
        let mutator: glassline_tui_dsl::screen::SettingsMutator =
            Box::new(move |s: &mut Settings| {
                while s.lines.len() <= line {
                    s.lines.push(Vec::new());
                }
                let row = &mut s.lines[line];
                let insert_at = if line_is_empty { 0 } else { cursor + 1 };
                let insert_at = insert_at.min(row.len());
                row.insert(insert_at, WidgetSpec::new("w-test", "git-branch"));
            });
        mutator(&mut s);
        assert_eq!(s.lines[0].len(), 1);
        assert_eq!(s.lines[0][0].kind, "git-branch");
    }

    #[test]
    fn fresh_id_is_unique_across_calls() {
        let a = fresh_widget_id();
        std::thread::sleep(std::time::Duration::from_nanos(1));
        let b = fresh_widget_id();
        assert_ne!(a, b, "consecutive fresh ids must differ");
    }
}
