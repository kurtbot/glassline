//! Compile-time widget-metadata registry. Every canonical widget in
//! [`glassline_widgets::registry::WIDGETS`] has a matching entry in
//! [`METAS`] here; the drift test (T2.7) enforces the two-way mapping.
//!
//! What lives here vs. what lives on the widget itself:
//! - **Widget code** carries render behaviour, requirements, defaults.
//! - **`WidgetMeta`** carries picker + editor UX metadata — the
//!   human-readable label, category, one-line description, and a list
//!   of knobs the editor should surface. It never runs in the render
//!   hot path.

pub mod drift;
pub mod entries;

pub use entries::METAS;

/// Editor + picker metadata for a single widget.
#[derive(Debug)]
pub struct WidgetMeta {
    /// The `type` string in `settings.json` — matches the phf-map key
    /// in `glassline_widgets::registry::WIDGETS`.
    pub id: &'static str,
    /// Human-readable name shown in the picker.
    pub label: &'static str,
    /// Grouping bucket in the picker.
    pub category: WidgetCategory,
    /// One-line description under the widget in the picker.
    pub description: &'static str,
    /// Whether the widget participates in the standard styling knobs
    /// (color / bold / dim / max_width). Markers like separators opt out.
    pub styling: Styling,
    /// Widget-specific knobs beyond the standard styling set.
    pub knobs: &'static [WidgetKnob],
}

/// Whether the widget participates in the standard styling knob set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Styling {
    /// Color + bold + dim + max-width are shown in the editor.
    Standard,
    /// Layout marker — no styling knobs (separator, flex-separator).
    Marker,
}

/// Top-level grouping in the widget picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetCategory {
    Model,
    Context,
    Tokens,
    Timing,
    Git,
    Jj,
    Session,
    Usage,
    Powerline,
    Custom,
    System,
    External,
}

impl WidgetCategory {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "Model",
            Self::Context => "Context",
            Self::Tokens => "Tokens",
            Self::Timing => "Timing",
            Self::Git => "Git",
            Self::Jj => "Jujutsu",
            Self::Session => "Session",
            Self::Usage => "Usage",
            Self::Powerline => "Powerline / Layout",
            Self::Custom => "Custom",
            Self::System => "System",
            Self::External => "External widgets",
        }
    }
}

/// A single knob the editor should surface for a widget.
#[derive(Debug)]
pub enum WidgetKnob {
    /// `spec.metadata[key]` with a typed value.
    Meta(MetaKnob),
    /// The widget's primary value — e.g. `custom-text.text`,
    /// `custom-symbol.symbol`.
    Value(ValueKnob),
    /// Escape hatch: opens a raw JSON editor for a specific spec
    /// field. Used for ext-widget knobs and gradient stops in v1.0.
    Raw(RawKnob),
}

/// A metadata knob: writes a `String` value into `spec.metadata[key]`.
#[derive(Debug)]
pub struct MetaKnob {
    pub key: &'static str,
    pub label: &'static str,
    pub shape: MetaShape,
}

/// The value shape a `MetaKnob` accepts. Screens use this to pick the
/// right editor sub-widget.
#[derive(Debug)]
pub enum MetaShape {
    /// Free-form text.
    Text { hint: &'static str, max_len: usize },
    /// `"true"` / `"false"` — treated as opt-in when absent.
    Bool { default_when_absent: bool },
    /// One of a fixed set of strings.
    Choice { options: &'static [&'static str] },
    /// Numeric value serialised as its decimal representation.
    Integer { min: u32, max: u32, default: u32 },
}

/// A knob that edits the widget's primary Value field.
#[derive(Debug)]
pub struct ValueKnob {
    pub label: &'static str,
    pub hint: &'static str,
    pub max_len: usize,
}

/// A knob that opens a raw JSON editor.
#[derive(Debug)]
pub struct RawKnob {
    pub label: &'static str,
}
