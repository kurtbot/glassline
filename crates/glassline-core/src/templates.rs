//! Baked-in `Settings` templates — three starter layouts glassline
//! ships out of the box.
//!
//! Living in `glassline-core` so **every** consumer can share one
//! source of truth:
//!   * `glassline-tui`'s first-run wizard offers these as the three
//!     template-pick options.
//!   * `glassline install` seeds [`power_user`] into the resolved
//!     settings.json path when the user has no config yet, so their
//!     Claude Code statusline works immediately after install without
//!     a wizard round-trip.
//!   * `glassline-tui --emit-screenshots` renders each template through
//!     the real pipeline for the README gallery.
//!
//! Every template calls [`Settings::in_memory_defaults`] for the
//! non-layout fields (color level, flex mode where applicable, powerline
//! defaults) and overrides only `lines` + fields the template
//! specifically wants to differ.

use crate::settings::{FlexMode, Settings, WidgetSpec};

/// A single line: cwd → git-branch → context-%. The gentlest possible
/// starter; users who want more add lines through the editor.
#[must_use]
pub fn minimal() -> Settings {
    Settings {
        lines: vec![vec![
            WidgetSpec::new("m1", "current-working-dir").with_color("blue"),
            WidgetSpec::new("m2", "separator"),
            WidgetSpec::new("m3", "git-branch").with_color("magenta"),
            WidgetSpec::new("m4", "separator"),
            WidgetSpec::new("m5", "context-percentage").with_color("yellow"),
        ]],
        ..Settings::in_memory_defaults()
    }
}

/// Two lines aimed at the mid-tier developer: model + git on line 1,
/// context + tokens + speed + compaction counter on line 2.
#[must_use]
pub fn dev() -> Settings {
    Settings {
        lines: vec![
            vec![
                WidgetSpec::new("d1", "model").with_color("cyan"),
                WidgetSpec::new("d2", "separator"),
                WidgetSpec::new("d3", "git-branch").with_color("magenta"),
                WidgetSpec::new("d4", "separator"),
                WidgetSpec::new("d5", "git-changes").with_color("brightGreen"),
            ],
            vec![
                WidgetSpec::new("d6", "context-percentage").with_color("yellow"),
                WidgetSpec::new("d7", "separator"),
                WidgetSpec::new("d8", "tokens-total").with_color("brightYellow"),
                WidgetSpec::new("d9", "separator"),
                WidgetSpec::new("d10", "total-speed").with_color("cyan"),
                WidgetSpec::new("d11", "separator"),
                WidgetSpec::new("d12", "compaction-counter").with_color("brightBlack"),
            ],
        ],
        ..Settings::in_memory_defaults()
    }
}

/// Three lines — the default `glassline install` seeds this so users
/// see a rich statusline immediately after install. Line 1: model +
/// context-bar + git + cwd. Line 2: context-% + session-clock +
/// weekly-reset + input/output speed. Line 3: session/weekly usage +
/// thinking-effort + session-cost. Uses `FullMinus40` flex mode to
/// keep lines from crowding out Claude Code's own prompt.
#[must_use]
pub fn power_user() -> Settings {
    Settings {
        lines: vec![
            vec![
                WidgetSpec::new("p1", "model").with_color("cyan"),
                WidgetSpec::new("p2", "separator"),
                WidgetSpec::new("p3", "context-bar").with_color("green"),
                WidgetSpec::new("p4", "separator"),
                WidgetSpec::new("p5", "git-branch").with_color("magenta"),
                WidgetSpec::new("p6", "separator"),
                WidgetSpec::new("p7", "git-changes").with_color("brightGreen"),
                WidgetSpec::new("p8", "separator"),
                WidgetSpec::new("p9", "current-working-dir").with_color("blue"),
            ],
            vec![
                WidgetSpec::new("p10", "context-percentage").with_color("yellow"),
                WidgetSpec::new("p11", "separator"),
                WidgetSpec::new("p12", "session-clock").with_color("yellow"),
                WidgetSpec::new("p13", "separator"),
                WidgetSpec::new("p14", "weekly-reset-timer").with_color("brightBlue"),
                WidgetSpec::new("p15", "separator"),
                WidgetSpec::new("p16", "input-speed").with_color("cyan"),
                WidgetSpec::new("p17", "separator"),
                WidgetSpec::new("p18", "output-speed").with_color("cyan"),
            ],
            vec![
                WidgetSpec::new("p19", "session-usage").with_color("brightGreen"),
                WidgetSpec::new("p20", "separator"),
                WidgetSpec::new("p21", "weekly-usage").with_color("brightCyan"),
                WidgetSpec::new("p22", "separator"),
                WidgetSpec::new("p23", "thinking-effort").with_color("magenta"),
                WidgetSpec::new("p24", "separator"),
                WidgetSpec::new("p25", "session-cost").with_color("brightYellow"),
            ],
        ],
        flex_mode: FlexMode::FullMinus40,
        ..Settings::in_memory_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_has_one_line() {
        let s = minimal();
        assert_eq!(s.lines.len(), 1);
        assert_eq!(s.lines[0].len(), 5);
    }

    #[test]
    fn dev_has_two_lines() {
        let s = dev();
        assert_eq!(s.lines.len(), 2);
    }

    #[test]
    fn power_user_has_three_lines_and_flex_minus_40() {
        let s = power_user();
        assert_eq!(s.lines.len(), 3);
        assert_eq!(s.flex_mode, FlexMode::FullMinus40);
    }

    #[test]
    fn power_user_starts_with_model_widget() {
        let s = power_user();
        assert_eq!(s.lines[0][0].kind, "model");
    }

    #[test]
    fn every_template_is_serializable() {
        // Regression guard: `install` seeds `power_user()` as JSON, so
        // any Settings field that isn't Serialize breaks that path.
        for s in [minimal(), dev(), power_user()] {
            let _ = serde_json::to_string_pretty(&s).expect("template must serialize");
        }
    }
}
