//! Multi-mode color picker modal — Basic16 palette, Ansi256 palette,
//! or Truecolor hex input. `Tab` cycles modes; `Enter` commits the
//! current mode's selection through the caller's `on_pick` callback.
//!
//! Committed strings match `glassline_core::protocol` color parsing:
//! - Named colors from Basic16 → `"red"`, `"brightBlue"`, …
//! - Ansi256 picks emit their nearest hex → `"#00afd7"` etc.
//! - Truecolor emits whatever the user typed (validated as `#rrggbb`).

use ratatui::{
    crossterm::event::{Event, KeyCode},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use glassline_tui_dsl::{Action, Screen, TextInput, Ui, centered_rect};

const NAMED_COLORS: &[(&str, Color)] = &[
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
    ("yellow", Color::Yellow),
    ("blue", Color::Blue),
    ("magenta", Color::Magenta),
    ("cyan", Color::Cyan),
    ("white", Color::Gray),
    ("brightBlack", Color::DarkGray),
    ("brightRed", Color::LightRed),
    ("brightGreen", Color::LightGreen),
    ("brightYellow", Color::LightYellow),
    ("brightBlue", Color::LightBlue),
    ("brightMagenta", Color::LightMagenta),
    ("brightCyan", Color::LightCyan),
    ("brightWhite", Color::White),
];

type OnPick = Box<dyn FnOnce(Option<String>) -> Action>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Basic16,
    Ansi256,
    Truecolor,
}

impl Mode {
    fn next(self) -> Self {
        match self {
            Self::Basic16 => Self::Ansi256,
            Self::Ansi256 => Self::Truecolor,
            Self::Truecolor => Self::Basic16,
        }
    }
}

pub struct ColorMenu {
    title_text: &'static str,
    mode: Mode,
    basic16_selected: usize,
    ansi256_selected: u8,
    truecolor_input: TextInput,
    on_pick: Option<OnPick>,
}

impl ColorMenu {
    pub fn new<F>(title: &'static str, current: Option<&str>, on_pick: F) -> Self
    where
        F: FnOnce(Option<String>) -> Action + 'static,
    {
        let basic16_selected = current
            .and_then(|name| NAMED_COLORS.iter().position(|(n, _)| *n == name))
            .unwrap_or(0);
        let mode = match current {
            Some(v) if v.starts_with('#') => Mode::Truecolor,
            _ => Mode::Basic16,
        };
        let truecolor_input = TextInput::new()
            .with_hint("#rrggbb (e.g. #ff8c00)")
            .with_max_len(7)
            .with_value(current.filter(|v| v.starts_with('#')).unwrap_or(""));
        Self {
            title_text: title,
            mode,
            basic16_selected,
            ansi256_selected: 0,
            truecolor_input,
            on_pick: Some(Box::new(on_pick)),
        }
    }

    fn current_pick(&self) -> Option<String> {
        match self.mode {
            Mode::Basic16 => Some(NAMED_COLORS[self.basic16_selected].0.to_string()),
            Mode::Ansi256 => Some(ansi256_to_hex(self.ansi256_selected)),
            Mode::Truecolor => {
                let v = self.truecolor_input.value().trim();
                if is_valid_hex(v) {
                    Some(v.to_string())
                } else {
                    None
                }
            }
        }
    }
}

impl Screen for ColorMenu {
    fn title(&self) -> &str {
        self.title_text
    }
    fn keybindings(&self) -> &[(&'static str, &'static str)] {
        match self.mode {
            Mode::Truecolor => &[
                ("type", "Hex"),
                ("Tab", "Mode"),
                ("Enter", "Pick"),
                ("Esc", "Cancel"),
            ],
            _ => &[
                ("↑/↓/←/→", "Nav"),
                ("Tab", "Mode"),
                ("Enter", "Pick"),
                ("Esc", "Cancel"),
            ],
        }
    }
    fn render(&mut self, ui: &mut Ui) {
        let area = centered_rect(ui.frame_area(), 70, 70);
        ui.frame.render_widget(ratatui::widgets::Clear, area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(self.title_text);
        let inner = block.inner(area);
        ui.render_widget(block, area);

        let [mode_row, body, hint] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        // Mode tabs.
        let tabs: Line = Line::from(vec![
            tab_span("Basic16", self.mode == Mode::Basic16),
            Span::raw("  "),
            tab_span("Ansi256", self.mode == Mode::Ansi256),
            Span::raw("  "),
            tab_span("Truecolor", self.mode == Mode::Truecolor),
            Span::styled("   [Tab to cycle]", dim()),
        ]);
        ui.render_widget(Paragraph::new(tabs), mode_row);

        match self.mode {
            Mode::Basic16 => render_basic16(body, ui, self.basic16_selected),
            Mode::Ansi256 => render_ansi256(body, ui, self.ansi256_selected),
            Mode::Truecolor => render_truecolor(body, ui, &self.truecolor_input),
        }

        let hint_line = match self.mode {
            Mode::Basic16 => format!("selected: {}", NAMED_COLORS[self.basic16_selected].0),
            Mode::Ansi256 => {
                let hex = ansi256_to_hex(self.ansi256_selected);
                format!("selected: #{} ({hex})", self.ansi256_selected)
            }
            Mode::Truecolor => {
                let v = self.truecolor_input.value();
                if is_valid_hex(v.trim()) {
                    format!("selected: {}", v.trim())
                } else {
                    "not a valid #rrggbb yet".into()
                }
            }
        };
        ui.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(hint_line, dim())])),
            hint,
        );
    }
    fn on_event(&mut self, ev: Event) -> Action {
        let Event::Key(k) = ev else {
            return Action::None;
        };
        // Tab always cycles modes regardless of focus.
        if k.code == KeyCode::Tab {
            self.mode = self.mode.next();
            return Action::None;
        }
        match self.mode {
            Mode::Basic16 => self.basic16_event(k),
            Mode::Ansi256 => self.ansi256_event(k),
            Mode::Truecolor => self.truecolor_event(k),
        }
    }
}

impl ColorMenu {
    fn basic16_event(&mut self, k: ratatui::crossterm::event::KeyEvent) -> Action {
        let cols = 4usize;
        let len = NAMED_COLORS.len();
        match k.code {
            KeyCode::Left => {
                if self.basic16_selected > 0 {
                    self.basic16_selected -= 1;
                }
                Action::None
            }
            KeyCode::Right => {
                if self.basic16_selected + 1 < len {
                    self.basic16_selected += 1;
                }
                Action::None
            }
            KeyCode::Up => {
                self.basic16_selected = self.basic16_selected.saturating_sub(cols);
                Action::None
            }
            KeyCode::Down => {
                let next = self.basic16_selected + cols;
                if next < len {
                    self.basic16_selected = next;
                }
                Action::None
            }
            KeyCode::Enter => self.commit(),
            KeyCode::Esc | KeyCode::Char('q') => self.cancel(),
            _ => Action::None,
        }
    }

    fn ansi256_event(&mut self, k: ratatui::crossterm::event::KeyEvent) -> Action {
        let cols: u8 = 16;
        match k.code {
            KeyCode::Left => {
                self.ansi256_selected = self.ansi256_selected.saturating_sub(1);
                Action::None
            }
            KeyCode::Right => {
                self.ansi256_selected = self.ansi256_selected.saturating_add(1);
                Action::None
            }
            KeyCode::Up => {
                self.ansi256_selected = self.ansi256_selected.saturating_sub(cols);
                Action::None
            }
            KeyCode::Down => {
                let next = self.ansi256_selected.saturating_add(cols);
                self.ansi256_selected = next;
                Action::None
            }
            KeyCode::Enter => self.commit(),
            KeyCode::Esc | KeyCode::Char('q') => self.cancel(),
            _ => Action::None,
        }
    }

    fn truecolor_event(&mut self, k: ratatui::crossterm::event::KeyEvent) -> Action {
        match k.code {
            KeyCode::Enter => {
                let v = self.truecolor_input.value().trim().to_string();
                if is_valid_hex(&v) {
                    self.commit()
                } else {
                    Action::Toast(format!("\"{v}\" is not a valid #rrggbb hex color"))
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => self.cancel(),
            _ => {
                self.truecolor_input.handle_event(&Event::Key(k));
                Action::None
            }
        }
    }

    fn commit(&mut self) -> Action {
        let pick = self.current_pick();
        let cb = self.on_pick.take().expect("commit fires once");
        Action::Sequence(vec![Action::Pop, cb(pick)])
    }

    fn cancel(&mut self) -> Action {
        if let Some(cb) = self.on_pick.take() {
            Action::Sequence(vec![Action::Pop, cb(None)])
        } else {
            Action::Pop
        }
    }
}

fn tab_span(label: &'static str, active: bool) -> Span<'static> {
    let style = if active {
        Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD)
    } else {
        Style::default().add_modifier(Modifier::DIM)
    };
    Span::styled(format!(" {label} "), style)
}

fn dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

fn render_basic16(area: Rect, ui: &mut Ui, selected: usize) {
    let rows = 4usize;
    let cols = 4usize;
    let row_areas: [Rect; 4] = Layout::vertical([
        Constraint::Ratio(1, rows as u32),
        Constraint::Ratio(1, rows as u32),
        Constraint::Ratio(1, rows as u32),
        Constraint::Ratio(1, rows as u32),
    ])
    .areas(area);
    for (r, row_area) in row_areas.iter().enumerate() {
        let cols_areas: [Rect; 4] = Layout::horizontal([
            Constraint::Ratio(1, cols as u32),
            Constraint::Ratio(1, cols as u32),
            Constraint::Ratio(1, cols as u32),
            Constraint::Ratio(1, cols as u32),
        ])
        .areas(*row_area);
        for (c, col_area) in cols_areas.iter().enumerate() {
            let idx = r * cols + c;
            if idx >= NAMED_COLORS.len() {
                continue;
            }
            let (name, color) = NAMED_COLORS[idx];
            let focused = idx == selected;
            let marker = if focused { " > " } else { "   " };
            let style = Style::default().fg(color).add_modifier(if focused {
                Modifier::BOLD | Modifier::REVERSED
            } else {
                Modifier::empty()
            });
            let line = Line::from(vec![
                Span::raw(marker),
                Span::styled(format!("███ {name}"), style),
            ]);
            ui.frame.render_widget(Paragraph::new(line), *col_area);
        }
    }
}

fn render_ansi256(area: Rect, ui: &mut Ui, selected: u8) {
    // 16 rows × 16 cols. Each cell renders one 3-wide colored block +
    // optional focus marker.
    let rows = 16usize;
    let row_h = (area.height.max(1) as usize).min(rows);
    if row_h == 0 {
        return;
    }
    let row_constraints: Vec<Constraint> = (0..rows).map(|_| Constraint::Length(1)).collect();
    let row_areas = Layout::vertical(row_constraints).split(area);
    let col_constraints: Vec<Constraint> = (0..16).map(|_| Constraint::Ratio(1, 16)).collect();
    for r in 0..rows.min(row_areas.len()) {
        let col_areas = Layout::horizontal(col_constraints.clone()).split(row_areas[r]);
        for c in 0..16 {
            let idx = (r * 16 + c) as u8;
            let focused = idx == selected;
            let color = Color::Indexed(idx);
            let style = Style::default().bg(color).fg(Color::White);
            let marker = if focused { "><" } else { "  " };
            let span = Span::styled(format!(" {marker} "), style);
            ui.frame
                .render_widget(Paragraph::new(Line::from(vec![span])), col_areas[c]);
        }
    }
}

fn render_truecolor(area: Rect, ui: &mut Ui, input: &TextInput) {
    let [hint_row, input_row, swatch, _spacer]: [Rect; 4] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Fill(1),
    ])
    .areas(area);
    ui.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "Type a hex color like #ff8c00 and press Enter.",
            dim(),
        )])),
        hint_row,
    );
    let block = Block::default().borders(Borders::ALL).title("hex");
    input.render(input_row, ui.frame, Some(block), true);
    // Swatch: paint a bar with the parsed color if valid.
    let v = input.value().trim();
    let (bg_style, label) = if let Some((r, g, b)) = parse_hex(v) {
        (
            Style::default().bg(Color::Rgb(r, g, b)),
            format!("preview swatch — {v}"),
        )
    } else {
        (
            Style::default().add_modifier(Modifier::DIM),
            "invalid hex — swatch hidden".into(),
        )
    };
    ui.frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(label, bg_style)])),
        swatch,
    );
}

/// Convert an ansi-256 palette index to its canonical hex.
/// Palette layout per xterm-256:
///   0..16   → standard 16 colors (approximate hex)
///   16..232 → 6×6×6 cube (each channel of `[0, 95, 135, 175, 215, 255]`)
///   232..256 → grayscale ramp (each step 8 + 10*i)
fn ansi256_to_hex(idx: u8) -> String {
    let (r, g, b) = if idx < 16 {
        BASIC16_HEX[idx as usize]
    } else if idx < 232 {
        let i = idx - 16;
        let steps = [0u8, 95, 135, 175, 215, 255];
        let ri = i / 36;
        let gi = (i / 6) % 6;
        let bi = i % 6;
        (steps[ri as usize], steps[gi as usize], steps[bi as usize])
    } else {
        let gray = 8u16 + 10u16 * u16::from(idx - 232);
        let g = gray.min(255) as u8;
        (g, g, g)
    };
    format!("#{r:02x}{g:02x}{b:02x}")
}

const BASIC16_HEX: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00),
    (0x80, 0x00, 0x00),
    (0x00, 0x80, 0x00),
    (0x80, 0x80, 0x00),
    (0x00, 0x00, 0x80),
    (0x80, 0x00, 0x80),
    (0x00, 0x80, 0x80),
    (0xc0, 0xc0, 0xc0),
    (0x80, 0x80, 0x80),
    (0xff, 0x00, 0x00),
    (0x00, 0xff, 0x00),
    (0xff, 0xff, 0x00),
    (0x00, 0x00, 0xff),
    (0xff, 0x00, 0xff),
    (0x00, 0xff, 0xff),
    (0xff, 0xff, 0xff),
];

fn is_valid_hex(v: &str) -> bool {
    v.len() == 7 && v.starts_with('#') && v[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn parse_hex(v: &str) -> Option<(u8, u8, u8)> {
    if !is_valid_hex(v) {
        return None;
    }
    let r = u8::from_str_radix(&v[1..3], 16).ok()?;
    let g = u8::from_str_radix(&v[3..5], 16).ok()?;
    let b = u8::from_str_radix(&v[5..7], 16).ok()?;
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use ratatui::crossterm::event::{Event, KeyEvent, KeyModifiers};

    use super::*;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }
    fn char_key(c: char) -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
    }

    #[test]
    fn initializes_at_named_color_index() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let menu = ColorMenu::new("Color", Some("cyan"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        assert_eq!(menu.basic16_selected, 6, "cyan is index 6");
        assert_eq!(menu.mode, Mode::Basic16);
    }

    #[test]
    fn initializes_at_truecolor_from_hex() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let menu = ColorMenu::new("Color", Some("#ff8c00"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        assert_eq!(menu.mode, Mode::Truecolor);
        assert_eq!(menu.truecolor_input.value(), "#ff8c00");
    }

    #[test]
    fn tab_cycles_mode() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", None, {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        assert_eq!(menu.mode, Mode::Basic16);
        menu.on_event(key(KeyCode::Tab));
        assert_eq!(menu.mode, Mode::Ansi256);
        menu.on_event(key(KeyCode::Tab));
        assert_eq!(menu.mode, Mode::Truecolor);
        menu.on_event(key(KeyCode::Tab));
        assert_eq!(menu.mode, Mode::Basic16);
    }

    #[test]
    fn ansi256_enter_emits_hex() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", None, {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        menu.on_event(key(KeyCode::Tab));
        assert_eq!(menu.mode, Mode::Ansi256);
        // Advance selection to a mid-palette entry.
        for _ in 0..16 {
            menu.on_event(key(KeyCode::Right));
        }
        assert_eq!(menu.ansi256_selected, 16);
        menu.on_event(key(KeyCode::Enter));
        let out = sink.lock().unwrap().clone().unwrap().unwrap();
        assert!(out.starts_with('#') && out.len() == 7, "got {out}");
    }

    #[test]
    fn truecolor_enter_accepts_valid_hex() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", None, {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        menu.on_event(key(KeyCode::Tab));
        menu.on_event(key(KeyCode::Tab));
        assert_eq!(menu.mode, Mode::Truecolor);
        for c in "#ff8c00".chars() {
            menu.on_event(char_key(c));
        }
        menu.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(Some("#ff8c00".into())));
    }

    #[test]
    fn truecolor_enter_rejects_invalid_hex_via_toast() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", None, {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        menu.on_event(key(KeyCode::Tab));
        menu.on_event(key(KeyCode::Tab));
        for c in "nope".chars() {
            menu.on_event(char_key(c));
        }
        let out = menu.on_event(key(KeyCode::Enter));
        assert!(matches!(out, Action::Toast(_)));
        assert!(sink.lock().unwrap().is_none(), "callback must not fire yet");
    }

    #[test]
    fn ansi256_hex_conversion_reference_points() {
        assert_eq!(ansi256_to_hex(0), "#000000");
        assert_eq!(ansi256_to_hex(15), "#ffffff");
        // Cube color 196 = pure red.
        assert_eq!(ansi256_to_hex(196), "#ff0000");
        // Grayscale ramp start.
        assert_eq!(ansi256_to_hex(232), "#080808");
        assert_eq!(ansi256_to_hex(255), "#eeeeee");
    }

    #[test]
    fn hex_validation() {
        assert!(is_valid_hex("#000000"));
        assert!(is_valid_hex("#ABCDEF"));
        assert!(!is_valid_hex("#12345"));
        assert!(!is_valid_hex("000000"));
        assert!(!is_valid_hex("#zzzzzz"));
    }

    #[test]
    fn basic16_enter_still_emits_name() {
        let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
        let mut menu = ColorMenu::new("Color", Some("red"), {
            let sink = sink.clone();
            move |c| {
                *sink.lock().unwrap() = Some(c);
                Action::Pop
            }
        });
        menu.on_event(key(KeyCode::Enter));
        assert_eq!(*sink.lock().unwrap(), Some(Some("red".into())));
    }

    #[test]
    fn esc_fires_none_in_any_mode() {
        for _ in 0..3 {
            let sink: Arc<Mutex<Option<Option<String>>>> = Arc::new(Mutex::new(None));
            let mut menu = ColorMenu::new("Color", None, {
                let sink = sink.clone();
                move |c| {
                    *sink.lock().unwrap() = Some(c);
                    Action::Pop
                }
            });
            menu.on_event(key(KeyCode::Tab));
            menu.on_event(key(KeyCode::Esc));
            assert_eq!(*sink.lock().unwrap(), Some(None));
        }
    }
}
