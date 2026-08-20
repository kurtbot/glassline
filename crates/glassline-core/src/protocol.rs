//! External-widget wire protocol v1 (design §4.11).
//!
//! `glassline` spawns each external widget as a child process, writes one
//! [`WidgetRequest`] as a single JSON line to stdin, and reads exactly one
//! [`WidgetResponse`] as a single JSON line from stdout. Widgets exit after
//! writing their response.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::color::Color;

/// The current wire protocol version glassline speaks. See design §4.15 for
/// the N + N-1 compatibility policy.
pub const CURRENT_PROTOCOL_VERSION: u32 = 1;

/// Everything sent to the widget's stdin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WidgetRequest {
    pub protocol_version: u32,
    /// Full widget ID as configured (`ext:foo`).
    pub widget_id: String,
    /// Opaque JSON blob the user set under `config` in `settings.json`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widget_config: Option<serde_json::Value>,
    pub context: WidgetContextPayload,
    /// Which optional capabilities glassline can honor in the response.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities_supported: Vec<String>,
    /// For future N + N-1 negotiation. In v1.0 we always send `[1]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protocol_versions_supported: Vec<u32>,
}

/// Subset of [`RenderContext`](crate::render_context::RenderContext) that the
/// widget receives. Deliberately smaller than the internal context — we don't
/// leak future fields we haven't stabilized on the wire.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WidgetContextPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_level: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub tokens: BTreeMap<String, u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
}

/// What the widget writes to stdout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WidgetResponse {
    pub protocol_version: u32,
    pub spans: Vec<WidgetSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_hint: Option<CacheHint>,
}

/// A single styled run of characters as sent over the wire.
///
/// Deliberately distinct from [`StyledSpan`](crate::span::StyledSpan): the
/// on-wire span carries colors as strings (`"#rrggbb"` / `"default"` / named)
/// while [`StyledSpan`] carries the parsed [`Color`] enum. Split lets the
/// wire schema evolve independently.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct WidgetSpan {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dim: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
}

/// Widget author's suggestion — glassline MAY reuse the response for
/// `reuse_ms` milliseconds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheHint {
    pub reuse_ms: u64,
}

/// Parse a color string emitted by a widget into the internal [`Color`] enum.
///
/// Formats accepted:
///  - `"default"` → [`Color::Default`]
///  - `"#rrggbb"` → [`Color::Rgb`]
///  - anything else → [`Color::Named`] (interpreted by the ANSI writer)
#[must_use]
pub fn parse_wire_color(input: Option<&str>) -> Color {
    let Some(s) = input else {
        return Color::Default;
    };
    if s.eq_ignore_ascii_case("default") {
        return Color::Default;
    }
    if let Some(hex) = s.strip_prefix('#')
        && hex.len() == 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        )
    {
        return Color::Rgb { r, g, b };
    }
    Color::Named(s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_color_default() {
        assert!(matches!(parse_wire_color(Some("default")), Color::Default));
        assert!(matches!(parse_wire_color(None), Color::Default));
    }

    #[test]
    fn wire_color_rgb() {
        assert!(matches!(
            parse_wire_color(Some("#0a80ff")),
            Color::Rgb {
                r: 0x0a,
                g: 0x80,
                b: 0xff,
            }
        ));
    }

    #[test]
    fn wire_color_named_fallback() {
        assert!(matches!(
            parse_wire_color(Some("brightGreen")),
            Color::Named(ref n) if n == "brightGreen"
        ));
    }

    #[test]
    fn wire_color_bad_hex_falls_back_to_named() {
        // Only 5 hex chars — not a valid `#rrggbb`; fall back to Named so a
        // widget bug never crashes the render pipeline.
        assert!(matches!(
            parse_wire_color(Some("#abcde")),
            Color::Named(ref n) if n == "#abcde"
        ));
    }

    #[test]
    fn request_round_trip() {
        let req = WidgetRequest {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            widget_id: "ext:git-worktrees".into(),
            widget_config: Some(serde_json::json!({"min_count":2})),
            context: WidgetContextPayload {
                session_id: Some("abc".into()),
                cwd: Some("/tmp".into()),
                ..Default::default()
            },
            capabilities_supported: vec!["styled_spans".into()],
            protocol_versions_supported: vec![1],
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: WidgetRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn response_round_trip() {
        let resp = WidgetResponse {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            spans: vec![
                WidgetSpan {
                    text: "wt:".into(),
                    fg: Some("#888888".into()),
                    ..Default::default()
                },
                WidgetSpan {
                    text: " 3".into(),
                    fg: Some("#00ff00".into()),
                    bold: true,
                    ..Default::default()
                },
            ],
            diagnostics: None,
            cache_hint: Some(CacheHint { reuse_ms: 5000 }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: WidgetResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, back);
    }

    #[test]
    fn response_defaults_when_flags_absent() {
        let resp: WidgetResponse =
            serde_json::from_str(r#"{"protocol_version":1,"spans":[{"text":"x"}]}"#).unwrap();
        assert_eq!(resp.spans.len(), 1);
        assert!(!resp.spans[0].bold);
        assert!(resp.diagnostics.is_none());
        assert!(resp.cache_hint.is_none());
    }
}
