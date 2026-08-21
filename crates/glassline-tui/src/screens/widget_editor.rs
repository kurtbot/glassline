//! `WidgetEditor` — a per-knob form for a single widget. Focus rows
//! are the standard color knob followed by widget-specific knobs from
//! the widget's [`WidgetMeta`]. Enter on the focused knob opens the
//! appropriate editor (color menu, text input, choice, integer).
//!
//! v1.0 scope: Color knob + `MetaShape::Text` / `Choice` / `Bool`
//! functional. `MetaShape::Integer` and Value knobs fall back to a
//! text-input modal. `Raw` opens a JSON editor (T3.11).

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use glassline_core::settings::{Settings, WidgetSpec};
use glassline_tui_dsl::{Action, Panel, Preview, Screen, Ui};

use crate::meta::{METAS, MetaKnob, MetaShape, Styling, WidgetKnob, WidgetMeta};
use crate::preview_ctx::canned_context;
use crate::screens::choice_modal::ChoiceModal;
use crate::screens::color_menu::ColorMenu;
use crate::screens::main_menu::preview_height;
use crate::screens::text_edit_modal::TextEditModal;

pub struct WidgetEditor {
    line_index: usize,
    widget_index: usize,
    focus: usize,
    /// Cached count of visible rows so we can wrap focus without
    /// re-querying WidgetMeta every keypress.
    last_row_count: usize,
    /// The rows list from the last render — cached so `on_event`
    /// (which doesn't have access to `Ui.settings`) can dispatch to
    /// the right knob editor for the focused row.
    cached_rows: Vec<CachedRow>,
}

/// Enough info about a row for `on_event` dispatch. Doesn't hold the
/// current value — the editor sub-modal re-reads from settings via a
/// MutateSettings closure.
#[derive(Debug, Clone)]
enum CachedRow {
    Color,
    Meta(&'static MetaKnob),
    Value,
    Raw,
}

impl WidgetEditor {
    #[must_use]
    pub fn new(line_index: usize, widget_index: usize) -> Self {
        Self {
            line_index,
            widget_index,
            focus: 0,
            last_row_count: 1,
            cached_rows: Vec::new(),
        }
    }

    fn spec<'a>(&self, settings: &'a Settings) -> Option<&'a WidgetSpec> {
        settings.lines.get(self.line_index)?.get(self.widget_index)
    }

    fn meta_for(&self, settings: &Settings) -> Option<&'static WidgetMeta> {
        let kind = self.spec(settings)?.kind.as_str();
        METAS.get(kind).copied()
    }
}

fn rows_for(spec: &WidgetSpec, meta: &WidgetMeta) -> Vec<Row> {
    let mut rows = Vec::new();
    if matches!(meta.styling, Styling::Standard) {
        rows.push(Row::Color(spec.color.clone()));
    }
    for knob in meta.knobs {
        rows.push(match knob {
            WidgetKnob::Meta(mk) => Row::Meta(mk, current_meta(spec, mk.key)),
            WidgetKnob::Value(_) => Row::Value(spec.custom_text.clone()),
            WidgetKnob::Raw(_) => Row::Raw,
        });
    }
    rows
}

fn current_meta(spec: &WidgetSpec, key: &str) -> Option<String> {
    spec.metadata.as_ref().and_then(|m| m.get(key)).cloned()
}

#[derive(Debug)]
enum Row {
    Color(Option<String>),
    Meta(&'static MetaKnob, Option<String>),
    Value(Option<String>),
    Raw,
}

impl Row {
    fn label(&self) -> String {
        match self {
            Self::Color(v) => format!("color = {}", v.as_deref().unwrap_or("(default)")),
            Self::Meta(mk, v) => format!("{} = {}", mk.label, v.as_deref().unwrap_or("(unset)")),
            Self::Value(v) => format!("value = {}", v.as_deref().unwrap_or("(unset)")),
            Self::Raw => "raw JSON …".into(),
        }
    }
}

impl Screen for WidgetEditor {
    fn title(&self) -> &str {
        "Widget editor"
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
        let Some(spec) = self.spec(ui.settings).cloned() else {
            self.last_row_count = 0;
            return;
        };
        let Some(meta) = self.meta_for(ui.settings) else {
            self.last_row_count = 0;
            return;
        };

        let rows = rows_for(&spec, meta);
        self.last_row_count = rows.len().max(1);
        if self.focus >= rows.len() {
            self.focus = rows.len().saturating_sub(1);
        }
        // Cache row-type info so on_event can dispatch without re-
        // querying WidgetMeta (and without needing Ui.settings).
        self.cached_rows = rows
            .iter()
            .map(|r| match r {
                Row::Color(_) => CachedRow::Color,
                Row::Meta(mk, _) => CachedRow::Meta(mk),
                Row::Value(_) => CachedRow::Value,
                Row::Raw => CachedRow::Raw,
            })
            .collect();

        let area = ui.area();
        let preview_h = preview_height(ui.settings.lines.len());
        let [preview_area, header, list_area, hint] = Layout::vertical([
            Constraint::Length(preview_h),
            Constraint::Length(2),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(area);

        Panel::new("Preview").render(preview_area, ui.frame, |inner, frame| {
            let settings = ui.settings.clone();
            let preview = Preview::new(canned_context, move || settings.clone());
            preview.render(inner, frame);
        });

        let header_text = format!("{}  ({})", meta.label, spec.kind);
        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                header_text,
                Style::default().add_modifier(Modifier::BOLD),
            )])),
            header,
        );

        Panel::new("Knobs").render(list_area, ui.frame, |inner, frame| {
            if rows.is_empty() {
                frame.render_widget(
                    Paragraph::new("This widget has no configuration.  Esc to go back.")
                        .style(Style::default().add_modifier(Modifier::DIM)),
                    inner,
                );
                return;
            }
            let lines: Vec<Line> = rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let marker = if i == self.focus { "> " } else { "  " };
                    let style = if i == self.focus {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    Line::from(vec![Span::raw(marker), Span::styled(r.label(), style)])
                })
                .collect();
            frame.render_widget(Paragraph::new(lines), inner);
        });

        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                meta.description,
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
                if self.focus + 1 < self.last_row_count {
                    self.focus += 1;
                }
                Action::None
            }
            KeyCode::Char(' ') => self.toggle_focused_bool(),
            KeyCode::Enter => self.activate_focused(),
            _ => Action::None,
        }
    }
}

impl WidgetEditor {
    fn activate_focused(&self) -> Action {
        let line = self.line_index;
        let widget = self.widget_index;
        let Some(row) = self.cached_rows.get(self.focus).cloned() else {
            return Action::None;
        };
        match row {
            CachedRow::Color => Action::Push(Box::new(ColorMenu::new(
                "Foreground color",
                None,
                move |pick| {
                    let Some(name) = pick else {
                        return Action::None;
                    };
                    Action::MutateSettings(Box::new(move |s| {
                        if let Some(row) = s.lines.get_mut(line)
                            && let Some(spec) = row.get_mut(widget)
                        {
                            spec.color = Some(name);
                        }
                    }))
                },
            ))),
            CachedRow::Value => Action::Push(Box::new(TextEditModal::new(
                "value",
                "text (custom-text / link label)",
                None,
                200,
                move |v| {
                    Action::MutateSettings(Box::new(move |s| {
                        if let Some(row) = s.lines.get_mut(line)
                            && let Some(spec) = row.get_mut(widget)
                        {
                            spec.custom_text = v;
                        }
                    }))
                },
            ))),
            CachedRow::Meta(mk) => dispatch_meta_knob(line, widget, mk),
            CachedRow::Raw => {
                Action::Toast("Raw JSON editor lands in T3.11. Use the file editor for now.".into())
            }
        }
    }

    fn toggle_focused_bool(&self) -> Action {
        let Some(CachedRow::Meta(mk)) = self.cached_rows.get(self.focus).cloned() else {
            return Action::None;
        };
        let MetaShape::Bool { .. } = mk.shape else {
            return Action::None;
        };
        let line = self.line_index;
        let widget = self.widget_index;
        Action::MutateSettings(Box::new(move |s| {
            if let Some(spec) = s.lines.get_mut(line).and_then(|row| row.get_mut(widget)) {
                toggle_bool_key(spec, mk.key);
            }
        }))
    }
}

/// Route a `MetaKnob` to the appropriate editor sub-modal.
fn dispatch_meta_knob(line: usize, widget: usize, mk: &'static MetaKnob) -> Action {
    match &mk.shape {
        MetaShape::Text { hint, max_len } => Action::Push(Box::new(TextEditModal::new(
            mk.label.to_string(),
            hint.to_string(),
            None,
            *max_len,
            move |v| write_meta_str(line, widget, mk.key, v),
        ))),
        MetaShape::Bool { .. } => Action::Toast(
            "Bool knob: press Space on the row to toggle (Enter is reserved for text/choice knobs)."
                .into(),
        ),
        MetaShape::Choice { options } => Action::Push(Box::new(ChoiceModal::new(
            mk.label.to_string(),
            options,
            None,
            move |v| write_meta_str(line, widget, mk.key, v),
        ))),
        MetaShape::Integer { min, max, default } => {
            let min = *min;
            let max = *max;
            let default = *default;
            Action::Push(Box::new(TextEditModal::new(
                mk.label.to_string(),
                format!("integer in [{min}, {max}], default {default}"),
                None,
                12,
                move |v| {
                    let Some(text) = v else {
                        return write_meta_str(line, widget, mk.key, None);
                    };
                    match text.parse::<u32>() {
                        Ok(n) if n >= min && n <= max => {
                            write_meta_str(line, widget, mk.key, Some(text))
                        }
                        _ => Action::Toast(format!(
                            "\"{text}\" is not a valid integer in [{min}, {max}]"
                        )),
                    }
                },
            )))
        }
    }
}

fn write_meta_str(line: usize, widget: usize, key: &'static str, value: Option<String>) -> Action {
    Action::MutateSettings(Box::new(move |s| {
        let Some(spec) = s.lines.get_mut(line).and_then(|row| row.get_mut(widget)) else {
            return;
        };
        let map = spec
            .metadata
            .get_or_insert_with(std::collections::BTreeMap::new);
        match value {
            Some(v) => {
                map.insert(key.to_string(), v);
            }
            None => {
                map.remove(key);
            }
        }
    }))
}

/// Flip the `"true"`/`"false"`/absent metadata triple. Absent → "true"
/// on first toggle; then round-trips normally.
fn toggle_bool_key(spec: &mut WidgetSpec, key: &str) {
    let meta = spec
        .metadata
        .get_or_insert_with(std::collections::BTreeMap::new);
    let current = meta.get(key).map(String::as_str);
    let next = match current {
        Some("true") => "false",
        Some("false") => "true",
        _ => "true",
    };
    meta.insert(key.to_string(), next.to_string());
}
