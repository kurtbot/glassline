//! SVG screenshot generation for the README gallery.
//!
//! Two entry points share one buffer→SVG serializer:
//!   * `emit_status_line` — pipes canned `RenderContext` + a caller-
//!     supplied `Settings` through `glassline_render::render_to_string`,
//!     parses the ANSI via `ansi-to-tui`, and paints the resulting
//!     `Text` onto a fresh cell grid.
//!   * `emit_editor_screen` — mounts one `Screen` in a `TestBackend`
//!     terminal, calls `render`, and dumps the buffer.
//!
//! Colors are resolved to hex through a compact xterm palette so the
//! output is self-contained (no CSS variables). Rects with the default
//! background are omitted to keep the SVG small.

use std::{fs, path::Path};

use ansi_to_tui::IntoText;
use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Text,
};

use glassline_core::settings::Settings;
use glassline_render::render_to_string;
use glassline_tui_dsl::{Screen, Ui};

use crate::preview_ctx::canned_context;
use crate::screens::{template_dev, template_power_user};

const CELL_W: u32 = 8;
const CELL_H: u32 = 16;
const FONT_SIZE: u32 = 13;
const BASELINE: u32 = 12;
const BG_DEFAULT: &str = "#0c0c0c";
const FG_DEFAULT: &str = "#c8c8c8";

/// Generate every README screenshot into `out_dir`. Creates the dir if
/// missing. Overwrites any existing SVGs. Returns filenames actually
/// written so the caller can log them.
pub fn generate_all(out_dir: &Path) -> Result<Vec<String>, String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("mkdir {}: {e}", out_dir.display()))?;
    let mut written = Vec::new();

    let dev = template_dev();
    let powerline = powerline_variant();

    written.push(emit_status_line(out_dir, "status-line-dev.svg", &dev)?);
    written.push(emit_status_line(
        out_dir,
        "status-line-powerline.svg",
        &powerline,
    )?);

    written.push(emit_editor_screen(
        out_dir,
        "editor-main-menu.svg",
        &dev,
        Box::new(crate::screens::MainMenu::new()),
        90,
        24,
    )?);
    written.push(emit_editor_screen(
        out_dir,
        "editor-widget-color.svg",
        &dev,
        Box::new(crate::screens::ColorMenu::new(
            "Widget color",
            Some("magenta"),
            |_| glassline_tui_dsl::Action::Pop,
        )),
        90,
        24,
    )?);
    written.push(emit_editor_screen(
        out_dir,
        "editor-wizard.svg",
        &Settings::default(),
        crate::screens::wizard_entry(),
        90,
        24,
    )?);

    Ok(written)
}

fn powerline_variant() -> Settings {
    let mut s = template_power_user();
    s.powerline.enabled = true;
    s.powerline.auto_align = true;
    s
}

fn emit_status_line(out_dir: &Path, filename: &str, settings: &Settings) -> Result<String, String> {
    let ctx = canned_context();
    let ansi = render_to_string(ctx, settings).map_err(|e| format!("render {filename}: {e}"))?;
    let text: Text<'_> = ansi
        .into_text()
        .map_err(|e| format!("ansi-to-tui {filename}: {e}"))?;

    let width = (text.width() as u32).max(80);
    let height = (text.lines.len() as u32).max(1);
    let svg = text_to_svg(&text, width, height);
    let path = out_dir.join(filename);
    fs::write(&path, svg).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(filename.to_string())
}

fn emit_editor_screen(
    out_dir: &Path,
    filename: &str,
    settings: &Settings,
    mut screen: Box<dyn Screen>,
    width: u16,
    height: u16,
) -> Result<String, String> {
    let backend = TestBackend::new(width, height);
    let mut terminal =
        Terminal::new(backend).map_err(|e| format!("mount TestBackend for {filename}: {e}"))?;
    terminal
        .draw(|frame| {
            let mut ui = Ui::new(frame, settings);
            screen.render(&mut ui);
        })
        .map_err(|e| format!("draw {filename}: {e}"))?;
    let buffer = terminal.backend().buffer().clone();
    let svg = buffer_to_svg(&buffer);
    let path = out_dir.join(filename);
    fs::write(&path, svg).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(filename.to_string())
}

/// Paint a ratatui `Text` (parsed ANSI) onto a `[cols × rows]` cell
/// grid and emit SVG. Cells beyond the text bounds stay at the default
/// background.
fn text_to_svg(text: &Text<'_>, cols: u32, rows: u32) -> String {
    let mut svg = svg_header(cols, rows);
    for (row_idx, line) in text.lines.iter().enumerate() {
        let mut col_idx: u32 = 0;
        for span in &line.spans {
            let style = span.style;
            for ch in span.content.chars() {
                paint_cell(&mut svg, col_idx, row_idx as u32, ch, style);
                col_idx += 1;
            }
        }
    }
    svg.push_str("</svg>\n");
    svg
}

/// Iterate every cell in a ratatui `Buffer` and emit SVG.
fn buffer_to_svg(buffer: &Buffer) -> String {
    let Rect { width, height, .. } = buffer.area;
    let mut svg = svg_header(width as u32, height as u32);
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            let ch = cell.symbol().chars().next().unwrap_or(' ');
            let style = Style::default()
                .fg(cell.fg)
                .bg(cell.bg)
                .add_modifier(cell.modifier);
            paint_cell(&mut svg, x as u32, y as u32, ch, style);
        }
    }
    svg.push_str("</svg>\n");
    svg
}

fn svg_header(cols: u32, rows: u32) -> String {
    let w = cols * CELL_W;
    let h = rows * CELL_H;
    let mut s = String::with_capacity(4096);
    s.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}" font-family="ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'DejaVu Sans Mono', monospace" font-size="{FONT_SIZE}">"#
    ));
    s.push_str(&format!(
        r#"<rect width="{w}" height="{h}" fill="{BG_DEFAULT}"/>"#
    ));
    s
}

fn paint_cell(svg: &mut String, col: u32, row: u32, ch: char, style: Style) {
    let reversed = style.add_modifier.contains(Modifier::REVERSED);
    let (fg_color, bg_color) = if reversed {
        (
            resolve_color_opt(style.bg, BG_DEFAULT),
            resolve_color_opt(style.fg, FG_DEFAULT),
        )
    } else {
        (
            resolve_color_opt(style.fg, FG_DEFAULT),
            resolve_color_opt(style.bg, BG_DEFAULT),
        )
    };
    let px = col * CELL_W;
    let py = row * CELL_H;
    if bg_color != BG_DEFAULT {
        svg.push_str(&format!(
            r#"<rect x="{px}" y="{py}" width="{CELL_W}" height="{CELL_H}" fill="{bg_color}"/>"#
        ));
    }
    if ch == ' ' || ch == '\0' {
        return;
    }
    let bold = style.add_modifier.contains(Modifier::BOLD);
    let italic = style.add_modifier.contains(Modifier::ITALIC);
    let dim = style.add_modifier.contains(Modifier::DIM);
    let mut attrs = format!(r#"x="{px}" y="{}" fill="{fg_color}""#, py + BASELINE);
    if bold {
        attrs.push_str(r#" font-weight="bold""#);
    }
    if italic {
        attrs.push_str(r#" font-style="italic""#);
    }
    if dim {
        attrs.push_str(r#" opacity="0.6""#);
    }
    let escaped = escape_xml(ch);
    svg.push_str(&format!(r#"<text {attrs}>{escaped}</text>"#));
}

fn resolve_color_opt(color: Option<Color>, default_hex: &str) -> String {
    match color {
        None => default_hex.to_string(),
        Some(c) => resolve_color(c, default_hex),
    }
}

fn resolve_color(color: Color, default_hex: &str) -> String {
    match color {
        Color::Reset => default_hex.to_string(),
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => xterm256_hex(i),
        Color::Black => xterm256_hex(0),
        Color::Red => xterm256_hex(1),
        Color::Green => xterm256_hex(2),
        Color::Yellow => xterm256_hex(3),
        Color::Blue => xterm256_hex(4),
        Color::Magenta => xterm256_hex(5),
        Color::Cyan => xterm256_hex(6),
        Color::Gray => xterm256_hex(7),
        Color::DarkGray => xterm256_hex(8),
        Color::LightRed => xterm256_hex(9),
        Color::LightGreen => xterm256_hex(10),
        Color::LightYellow => xterm256_hex(11),
        Color::LightBlue => xterm256_hex(12),
        Color::LightMagenta => xterm256_hex(13),
        Color::LightCyan => xterm256_hex(14),
        Color::White => xterm256_hex(15),
    }
}

fn xterm256_hex(i: u8) -> String {
    const BASE16: [(u8, u8, u8); 16] = [
        (0x0c, 0x0c, 0x0c),
        (0xc5, 0x0f, 0x1f),
        (0x13, 0xa1, 0x0e),
        (0xc1, 0x9c, 0x00),
        (0x00, 0x37, 0xda),
        (0x88, 0x17, 0x98),
        (0x3a, 0x96, 0xdd),
        (0xcc, 0xcc, 0xcc),
        (0x76, 0x76, 0x76),
        (0xe7, 0x48, 0x56),
        (0x16, 0xc6, 0x0c),
        (0xf9, 0xf1, 0xa5),
        (0x3b, 0x78, 0xff),
        (0xb4, 0x00, 0x9e),
        (0x61, 0xd6, 0xd6),
        (0xf2, 0xf2, 0xf2),
    ];
    const CUBE_STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    if i < 16 {
        let (r, g, b) = BASE16[i as usize];
        return format!("#{r:02x}{g:02x}{b:02x}");
    }
    if i < 232 {
        let n = i - 16;
        let r = CUBE_STEPS[(n / 36) as usize];
        let g = CUBE_STEPS[((n / 6) % 6) as usize];
        let b = CUBE_STEPS[(n % 6) as usize];
        return format!("#{r:02x}{g:02x}{b:02x}");
    }
    let v = (i - 232) * 10 + 8;
    format!("#{v:02x}{v:02x}{v:02x}")
}

fn escape_xml(ch: char) -> String {
    match ch {
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '&' => "&amp;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        c => c.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xterm256_named_colors_match_palette() {
        assert_eq!(xterm256_hex(1), "#c50f1f");
        assert_eq!(xterm256_hex(2), "#13a10e");
        assert_eq!(xterm256_hex(15), "#f2f2f2");
    }

    #[test]
    fn xterm256_grayscale_ramp_last_entry() {
        assert_eq!(xterm256_hex(255), "#eeeeee");
    }

    #[test]
    fn escape_xml_reserves_are_escaped() {
        assert_eq!(escape_xml('<'), "&lt;");
        assert_eq!(escape_xml('&'), "&amp;");
        assert_eq!(escape_xml('a'), "a");
    }

    #[test]
    fn powerline_variant_enables_powerline() {
        let s = powerline_variant();
        assert!(s.powerline.enabled);
        assert!(s.powerline.auto_align);
    }
}
