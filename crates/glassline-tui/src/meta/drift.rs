//! Compile-time-ish drift test between
//! `glassline_widgets::registry::WIDGETS` and this crate's `METAS`.
//!
//! Contract:
//! - Every **canonical** widget id in the factory registry has a
//!   `WidgetMeta` entry here.
//! - Every `METAS` key is registered as a widget.
//! - `METAS` keys are **canonical only** — aliases are excluded (they
//!   inherit their canonical widget's metadata).
//!
//! A widget added to the factory registry without a matching META
//! fails CI here; an alias sneaking into `METAS` fails here too.

#[cfg(test)]
mod tests {
    use crate::meta::METAS;
    use glassline_widgets::registry::{ALIASES, WIDGETS};

    #[test]
    fn every_canonical_widget_has_meta() {
        for (id, _) in WIDGETS.entries() {
            if ALIASES.contains(id) {
                continue;
            }
            assert!(
                METAS.get(id).is_some(),
                "canonical widget {id:?} is registered but has no WidgetMeta entry"
            );
        }
    }

    #[test]
    fn every_meta_id_is_a_registered_canonical() {
        for (id, _) in METAS.entries() {
            assert!(
                WIDGETS.get(id).is_some(),
                "META entry {id:?} has no matching factory in registry::WIDGETS"
            );
            assert!(
                !ALIASES.contains(id),
                "META entry {id:?} is an alias — aliases must inherit their canonical's metadata"
            );
        }
    }

    #[test]
    fn meta_id_field_matches_map_key() {
        // Catches accidental copy-paste where an entry's `id:` doesn't
        // match its map key.
        for (key, meta) in METAS.entries() {
            assert_eq!(
                *key, meta.id,
                "METAS key {key:?} maps to a WidgetMeta whose id is {:?}",
                meta.id
            );
        }
    }

    #[test]
    fn meta_labels_are_nonempty() {
        for (_key, meta) in METAS.entries() {
            assert!(
                !meta.label.is_empty(),
                "widget {:?} has an empty label",
                meta.id
            );
            assert!(
                !meta.description.is_empty(),
                "widget {:?} has an empty description",
                meta.id
            );
        }
    }

    #[test]
    fn canonical_count_matches_meta_count() {
        // Belt-and-braces: the two counts must be identical once
        // aliases are subtracted.
        let canonical: usize = WIDGETS
            .entries()
            .filter(|(id, _)| !ALIASES.contains(id))
            .count();
        assert_eq!(
            canonical,
            METAS.len(),
            "canonical widget count ({canonical}) != METAS count ({})",
            METAS.len(),
        );
    }
}
