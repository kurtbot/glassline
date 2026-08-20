//! `WidgetPicker` — a filter-as-you-type list over the `METAS` catalog,
//! grouped by [`WidgetCategory`]. Enter fires a caller-supplied
//! callback with the chosen widget id and pops the screen.
//!
//! Iteration source is `METAS`, not the factory registry: aliases
//! inherit their canonical's metadata, so aliases must not appear as
//! standalone picker rows.

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use glassline_tui_dsl::{Action, List, Panel, Screen, TextInput, Ui};

use crate::meta::{METAS, WidgetMeta};

/// Callback type — invoked when the user activates a widget row. The
/// picker forwards whatever `Action` the callback returns; typical
/// screens use `Action::Pop` and stash the chosen id via captured
/// state.
type OnSelect = Box<dyn FnMut(&'static WidgetMeta) -> Action>;

/// Picker screen.
pub struct WidgetPicker {
    list: List<&'static WidgetMeta>,
    filter: TextInput,
    on_select: OnSelect,
}

impl WidgetPicker {
    /// Build a picker that fires `on_select` with the highlighted
    /// widget when the user presses Enter. Rows are pre-sorted by
    /// category then by label so the visual order is stable.
    pub fn new<F>(on_select: F) -> Self
    where
        F: FnMut(&'static WidgetMeta) -> Action + 'static,
    {
        let mut entries: Vec<&'static WidgetMeta> =
            METAS.entries().map(|(_, meta)| *meta).collect();
        entries.sort_by(|a, b| {
            let ca = a.category as u32;
            let cb = b.category as u32;
            ca.cmp(&cb).then_with(|| a.label.cmp(b.label))
        });
        Self {
            list: List::new(entries),
            filter: TextInput::new()
                .with_hint("filter widgets…")
                .with_max_len(60),
            on_select: Box::new(on_select),
        }
    }

    /// The currently-highlighted widget in the filtered subset, or
    /// `None` when the filter matches nothing.
    #[must_use]
    pub fn highlighted(&self) -> Option<&'static WidgetMeta> {
        self.list.selected_item(picker_label).copied()
    }
}

/// Label used for both the visible list row and the filter query.
/// Format: `"<category>  <label>  (<id>)"` — so filtering by id or by
/// human label both work naturally.
fn picker_label(meta: &&'static WidgetMeta) -> String {
    format!("{}  {}  ({})", meta.category.label(), meta.label, meta.id)
}

impl Screen for WidgetPicker {
    fn title(&self) -> &str {
        "Pick a widget"
    }

    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        &[
            ("↑/↓", "Nav"),
            ("type", "Filter"),
            ("Enter", "Pick"),
            ("Esc", "Cancel"),
        ]
    }

    fn render(&mut self, ui: &mut Ui) {
        let area = ui.area();
        let [filter_row, list_area, hint_row] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        let filter_block = Block::default()
            .borders(Borders::ALL)
            .title("filter (type to narrow)");
        self.filter
            .render(filter_row, ui.frame, Some(filter_block), true);

        // Push the current filter through in case on_event didn't
        // (e.g. after a set_items or programmatic mutation).
        self.list.set_filter(self.filter.value());

        Panel::new("Widgets").render(list_area, ui.frame, |inner, frame| {
            self.list.render(inner, frame, picker_label);
        });

        // Hint row shows the description of the highlighted widget.
        let hint = self
            .highlighted()
            .map(|m| format!("{}: {}", m.id, m.description))
            .unwrap_or_else(|| "no match — try a different filter".to_string());
        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                hint,
                Style::default().add_modifier(Modifier::DIM),
            )])),
            hint_row,
        );
    }

    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        match k.code {
            KeyCode::Esc => Action::Pop,
            KeyCode::Enter => {
                if let Some(meta) = self.highlighted() {
                    // Always pop the picker after firing the callback
                    // so callers don't have to wrap their return in a
                    // Sequence(Pop, ...) themselves.
                    let cb_action = (self.on_select)(meta);
                    return Action::Sequence(vec![Action::Pop, cb_action]);
                }
                Action::None
            }
            KeyCode::Up => {
                self.list.move_up(picker_label);
                Action::None
            }
            KeyCode::Down => {
                self.list.move_down(picker_label);
                Action::None
            }
            _ => {
                // Everything else goes to the filter buffer.
                self.filter.handle_event(&Event::Key(k));
                // Push the new filter through immediately so
                // `highlighted()` reflects the updated subset without
                // waiting for the next render tick.
                self.list.set_filter(self.filter.value());
                Action::None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::*;

    fn key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }
    fn special(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn callback_recording(sink: Arc<Mutex<Vec<&'static str>>>) -> OnSelect {
        Box::new(move |meta: &'static WidgetMeta| {
            sink.lock().unwrap().push(meta.id);
            Action::Pop
        })
    }

    #[test]
    fn opens_with_first_item_highlighted() {
        let sink = Arc::new(Mutex::new(Vec::new()));
        let picker = WidgetPicker::new({
            let sink = sink.clone();
            move |m: &'static WidgetMeta| {
                sink.lock().unwrap().push(m.id);
                Action::Pop
            }
        });
        assert!(picker.highlighted().is_some());
    }

    #[test]
    fn enter_invokes_callback_with_highlighted_widget() {
        let sink: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let mut picker = WidgetPicker {
            list: {
                let mut entries: Vec<&'static WidgetMeta> =
                    METAS.entries().map(|(_, m)| *m).collect();
                entries.sort_by(|a, b| a.label.cmp(b.label));
                List::new(entries)
            },
            filter: TextInput::new(),
            on_select: callback_recording(sink.clone()),
        };
        // Enter — should fire callback with the first item.
        let expected = picker.highlighted().unwrap().id;
        let _ = picker.on_event(special(KeyCode::Enter));
        let recorded = sink.lock().unwrap().clone();
        assert_eq!(recorded, vec![expected]);
    }

    #[test]
    fn typing_narrows_filter() {
        let sink: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let mut picker = WidgetPicker::new({
            let sink = sink.clone();
            move |m: &'static WidgetMeta| {
                sink.lock().unwrap().push(m.id);
                Action::Pop
            }
        });
        for c in "vim".chars() {
            picker.on_event(key(c));
        }
        // After typing "vim", the highlighted widget must contain "vim"
        // somewhere in its label/id.
        let hi = picker.highlighted().unwrap();
        let hay = picker_label(&hi).to_ascii_lowercase();
        assert!(
            hay.contains("vim"),
            "expected label containing 'vim': {hay}"
        );
    }

    #[test]
    fn esc_pops() {
        let sink: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let mut picker = WidgetPicker::new({
            let sink = sink.clone();
            move |m: &'static WidgetMeta| {
                sink.lock().unwrap().push(m.id);
                Action::Pop
            }
        });
        assert!(matches!(
            picker.on_event(special(KeyCode::Esc)),
            Action::Pop
        ));
        assert!(
            sink.lock().unwrap().is_empty(),
            "Esc must not fire on_select"
        );
    }

    #[test]
    fn empty_filter_result_yields_no_highlighted() {
        let sink: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));
        let mut picker = WidgetPicker::new({
            let sink = sink.clone();
            move |m: &'static WidgetMeta| {
                sink.lock().unwrap().push(m.id);
                Action::Pop
            }
        });
        for c in "zzzznever".chars() {
            picker.on_event(key(c));
        }
        assert!(picker.highlighted().is_none());
        // Enter with nothing highlighted is a no-op.
        let _ = picker.on_event(special(KeyCode::Enter));
        assert!(sink.lock().unwrap().is_empty());
    }
}
