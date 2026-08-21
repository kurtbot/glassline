//! Bordered container primitive. Takes a title + a border style,
//! renders a ratatui [`Block`] into the given area, then hands the
//! computed inner area back to the caller so children can be laid out.
//!
//! Children are deliberately caller-provided rather than owned by the
//! `Panel` — that keeps the primitive dependency-free and side-steps
//! the "what type is a child" question until a wider `Element` type
//! actually pays off.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders},
};

/// Border corner + line style. Matches the four ratatui
/// [`BorderType`] variants; wrapped so screens don't reach into
/// ratatui directly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorderStyle {
    #[default]
    Plain,
    Rounded,
    Double,
    Thick,
}

impl From<BorderStyle> for BorderType {
    fn from(v: BorderStyle) -> Self {
        match v {
            BorderStyle::Plain => BorderType::Plain,
            BorderStyle::Rounded => BorderType::Rounded,
            BorderStyle::Double => BorderType::Double,
            BorderStyle::Thick => BorderType::Thick,
        }
    }
}

/// Bordered panel with a title and a caller-provided inner render.
#[derive(Debug, Clone, Copy)]
pub struct Panel<'a> {
    title: &'a str,
    border: BorderStyle,
    style: Style,
}

impl<'a> Panel<'a> {
    #[must_use]
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            border: BorderStyle::Plain,
            style: Style::default(),
        }
    }

    #[must_use]
    pub fn with_border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    #[must_use]
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Draw the panel chrome into `area`, then call `render_children`
    /// with the inner rect + frame for the caller to fill.
    ///
    /// When `area` is too small for even the borders + title, the
    /// closure still fires with a possibly-empty inner rect — screens
    /// can guard on `area.width < 2 || area.height < 2` themselves.
    pub fn render<F>(self, area: Rect, frame: &mut Frame, render_children: F)
    where
        F: FnOnce(Rect, &mut Frame),
    {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(self.border.into())
            .title(self.title)
            .style(self.style);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        render_children(inner, frame);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn plain_border_default() {
        assert_eq!(Panel::new("t").border, BorderStyle::Plain);
    }

    #[test]
    fn builder_sets_border_style() {
        let p = Panel::new("t").with_border(BorderStyle::Rounded);
        assert_eq!(p.border, BorderStyle::Rounded);
    }

    #[test]
    fn renders_bordered_block_with_title_into_test_backend() {
        let backend = TestBackend::new(12, 4);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|frame| {
            Panel::new("Hi").render(frame.area(), frame, |inner, _f| {
                // Inner should be at least the outer area minus 2 in
                // each dimension (borders).
                assert!(inner.width <= 10);
                assert!(inner.height <= 2);
            });
        })
        .unwrap();
        // Line 0 has a title. Line 3 is the bottom border.
        let buf = term.backend().buffer();
        let top_row: String = (0..12).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(
            top_row.contains("Hi"),
            "top border should carry the title; got {top_row:?}"
        );
    }

    #[test]
    fn tiny_area_still_fires_render_children() {
        // 1×1 backend — panel can't draw borders + title, but the
        // closure must still fire so downstream code doesn't skip
        // state updates.
        let backend = TestBackend::new(1, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut fired = false;
        term.draw(|frame| {
            Panel::new("x").render(frame.area(), frame, |_inner, _f| {
                fired = true;
            });
        })
        .unwrap();
        assert!(fired, "render_children must always be called");
    }

    #[test]
    fn border_style_maps_to_ratatui() {
        assert_eq!(BorderType::from(BorderStyle::Plain), BorderType::Plain);
        assert_eq!(BorderType::from(BorderStyle::Rounded), BorderType::Rounded);
        assert_eq!(BorderType::from(BorderStyle::Double), BorderType::Double);
        assert_eq!(BorderType::from(BorderStyle::Thick), BorderType::Thick);
    }
}
