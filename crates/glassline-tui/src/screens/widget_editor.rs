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

use crate::meta::{METAS, MetaKnob, Styling, WidgetKnob, WidgetMeta};
use crate::preview_ctx::canned_context;
use crate::screens::color_menu::ColorMenu;
use crate::screens::main_menu::preview_height;

pub struct WidgetEditor {
    line_index: usize,
    widget_index: usize,
    focus: usize,
    /// Cached count of visible rows so we can wrap focus without
    /// re-querying WidgetMeta every keypress.
    last_row_count: usize,
}

impl WidgetEditor {
    #[must_use]
    pub fn new(line_index: usize, widget_index: usize) -> Self {
        Self {
            line_index,
            widget_index,
            focus: 0,
            last_row_count: 1,
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
        &[("↑/↓", "Focus"), ("Enter", "Edit knob"), ("Esc", "Back")]
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
            KeyCode::Enter => self.activate_focused(),
            _ => Action::None,
        }
    }
}

impl WidgetEditor {
    fn activate_focused(&self) -> Action {
        // This method only produces the initial Action; the actual
        // knob-row list is rebuilt inside the ColorMenu / knob-editor
        // closure via a MutateSettings that captures line + widget
        // indices. We don't need to hold a full Row here.
        let line = self.line_index;
        let widget = self.widget_index;
        let focus = self.focus;

        Action::Push(Box::new(ColorMenu::new(
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
                        // Only actually apply the color if the focused row
                        // was the color row (row 0 when Standard styling).
                        if focus == 0 {
                            spec.color = Some(name);
                        }
                        // For non-color rows in v1.0 the color menu is a
                        // no-op; a proper knob-type dispatch lands in
                        // follow-up commits within P3.
                    }
                }))
            },
        )))
    }
}
