//! `link` — renders an ANSI OSC 8 hyperlink wrapping user-supplied text.
//! Port of upstream `Link.tsx`.
//!
//! Config via `spec.metadata`:
//! - `url` — required. The hyperlink target.
//! - `text` — display text. Falls back to `spec.custom_text`, then to
//!   the URL itself.
//!
//! Terminal support: OSC 8 is widely supported in modern terminals
//! (Windows Terminal, iTerm2, WezTerm, kitty, Alacritty, VS Code
//! integrated terminal). Terminals that don't understand it render the
//! escape as an invisible no-op, leaving just the display text.

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};

use crate::common::styled;

pub fn factory() -> Box<dyn Widget> {
    Box::new(LinkWidget)
}

pub struct LinkWidget;

impl Widget for LinkWidget {
    fn id(&self) -> &'static str {
        "link"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    fn render(&self, spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(url) = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("url"))
            .map(String::as_str)
            .filter(|s| !s.is_empty())
        else {
            return Vec::new();
        };
        let text = spec
            .metadata
            .as_ref()
            .and_then(|m| m.get("text"))
            .map(String::as_str)
            .or(spec.custom_text.as_deref())
            .unwrap_or(url);
        // OSC 8: `\x1b]8;;URL\x07TEXT\x1b]8;;\x07`
        let wrapped = format!("\x1b]8;;{url}\x07{text}\x1b]8;;\x07");
        styled(spec, wrapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn spec_with(meta: &[(&str, &str)]) -> WidgetSpec {
        let mut s = WidgetSpec::new("1", "link");
        let m: BTreeMap<String, String> = meta
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        s.metadata = Some(m);
        s
    }

    #[test]
    fn renders_osc8_wrapped_link() {
        let spec = spec_with(&[("url", "https://example.com"), ("text", "example")]);
        let spans = LinkWidget.render(&spec, &RenderContext::default());
        assert_eq!(spans.len(), 1);
        assert_eq!(
            spans[0].text,
            "\x1b]8;;https://example.com\x07example\x1b]8;;\x07"
        );
    }

    #[test]
    fn empty_when_url_missing() {
        let spec = spec_with(&[("text", "no-url")]);
        let spans = LinkWidget.render(&spec, &RenderContext::default());
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_url_is_empty_string() {
        let spec = spec_with(&[("url", ""), ("text", "hi")]);
        let spans = LinkWidget.render(&spec, &RenderContext::default());
        assert!(spans.is_empty());
    }

    #[test]
    fn falls_back_to_custom_text_when_no_text_meta() {
        let mut spec = spec_with(&[("url", "https://example.com")]);
        spec.custom_text = Some("from-custom-text".to_string());
        let spans = LinkWidget.render(&spec, &RenderContext::default());
        assert!(spans[0].text.contains("from-custom-text"));
    }

    #[test]
    fn falls_back_to_url_when_no_text_at_all() {
        let spec = spec_with(&[("url", "https://example.com")]);
        let spans = LinkWidget.render(&spec, &RenderContext::default());
        // Display text ends up == url; the escape sandwich contains url twice.
        assert!(spans[0].text.starts_with("\x1b]8;;https://example.com\x07"));
        assert!(spans[0].text.contains("https://example.com\x1b]8;;\x07"));
    }
}
