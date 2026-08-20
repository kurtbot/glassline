//! [`Widget`] trait + [`WidgetRequirements`] bitset.
//!
//! Each widget declares the expensive contexts it depends on so the render
//! binary can skip transcript parsing / git shell-outs / usage HTTP when
//! nothing on the visible lines needs them (mirror of TS's
//! `hasSpeedItems` / `hasCompactionWidget` / `hasSessionClock` scan in
//! `ccstatusline.ts`).

use crate::{render_context::RenderContext, settings::WidgetSpec, span::StyledSpan};

/// A widget's runtime data dependencies. Bitwise-OR the requirements of every
/// widget on every visible line; call the corresponding data collector only
/// for the flags that end up set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WidgetRequirements {
    bits: u16,
}

impl WidgetRequirements {
    pub const NONE: Self = Self { bits: 0 };
    pub const TRANSCRIPT: Self = Self { bits: 1 << 0 };
    pub const GIT: Self = Self { bits: 1 << 1 };
    pub const GIT_REVIEW: Self = Self { bits: 1 << 2 };
    pub const USAGE: Self = Self { bits: 1 << 3 };
    pub const SPEED: Self = Self { bits: 1 << 4 };
    pub const COMPACTION: Self = Self { bits: 1 << 5 };
    pub const SKILLS: Self = Self { bits: 1 << 6 };
    pub const SESSION_CLOCK: Self = Self { bits: 1 << 7 };
    pub const BLOCK: Self = Self { bits: 1 << 8 };
    pub const CACHE: Self = Self { bits: 1 << 9 };

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

impl std::ops::BitOr for WidgetRequirements {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for WidgetRequirements {
    fn bitor_assign(&mut self, rhs: Self) {
        self.bits |= rhs.bits;
    }
}

impl std::ops::BitAnd for WidgetRequirements {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self {
            bits: self.bits & rhs.bits,
        }
    }
}

/// A widget produces spans given a spec + context. See design §4.2.
pub trait Widget: Send + Sync {
    /// Stable widget ID as it appears in `settings.json` (`type` field).
    /// Built-in widgets return a bare kebab-case string (`"git-branch"`);
    /// external widgets return their `ext:*` prefix.
    fn id(&self) -> &'static str;

    /// Runtime data this widget needs. Union of these across all visible
    /// widgets decides which collectors run.
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    /// Default fg color when the widget's `spec.color` is not set.
    /// Matches TS `getDefaultColor()` on each widget. Returning `None`
    /// leaves the fg unspecified (defaults to the terminal default,
    /// which in Claude Code's dimmed status area reads as gray).
    fn default_color(&self) -> Option<&'static str> {
        None
    }

    /// Produce a run of styled spans. Empty result → widget hides itself
    /// (renderer skips it). `None` currently isn't distinct from empty vec;
    /// we return `Vec<StyledSpan>` for simplicity.
    fn render(&self, spec: &WidgetSpec, ctx: &RenderContext) -> Vec<StyledSpan>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_default_is_empty() {
        let r = WidgetRequirements::default();
        assert_eq!(r, WidgetRequirements::NONE);
    }

    #[test]
    fn requirements_union_and_contains() {
        let combined = WidgetRequirements::GIT | WidgetRequirements::USAGE;
        assert!(combined.contains(WidgetRequirements::GIT));
        assert!(combined.contains(WidgetRequirements::USAGE));
        assert!(!combined.contains(WidgetRequirements::TRANSCRIPT));
    }

    #[test]
    fn requirements_or_assign() {
        let mut r = WidgetRequirements::default();
        r |= WidgetRequirements::TRANSCRIPT;
        r |= WidgetRequirements::SPEED;
        assert!(r.contains(WidgetRequirements::TRANSCRIPT));
        assert!(r.contains(WidgetRequirements::SPEED));
        assert!(!r.contains(WidgetRequirements::GIT));
    }
}
