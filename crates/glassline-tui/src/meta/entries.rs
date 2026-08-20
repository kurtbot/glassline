//! The `METAS` phf::Map — one static entry per canonical widget id.
//! Populated in T2.4-T2.6. The drift test (T2.7) fails until every
//! widget in `glassline_widgets::registry::WIDGETS` has a matching
//! entry here (excluding aliases).

use phf::phf_map;

use super::WidgetMeta;

/// Registry keyed by the same `type` string as
/// `glassline_widgets::registry::WIDGETS`. Aliases are excluded —
/// they inherit their canonical widget's metadata.
pub static METAS: phf::Map<&'static str, &'static WidgetMeta> = phf_map! {};
