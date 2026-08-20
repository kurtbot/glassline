//! Color primitives.
//!
//! Ports [`src/types/ColorLevel.ts`](https://github.com/sirmalloc/ccstatusline)
//! and [`src/types/ColorEntry.ts`](https://github.com/sirmalloc/ccstatusline).
//!
//! [`Color`] carries the raw palette info; the render layer decides how to
//! encode it in ANSI SGR based on the active [`ColorLevel`].

use serde::{Deserialize, Serialize};

/// Chalk color level (design §4.4 replicates chalk's byte output).
///
/// Values match TS `ColorLevelSchema` — `0..=3`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[repr(u8)]
#[serde(from = "u8", into = "u8")]
pub enum ColorLevel {
    /// No colors.
    None = 0,
    /// Basic 16 colors.
    Ansi16 = 1,
    /// 256 colors.
    #[default]
    Ansi256 = 2,
    /// Truecolor (24-bit RGB).
    Truecolor = 3,
}

impl From<u8> for ColorLevel {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::None,
            1 => Self::Ansi16,
            3 => Self::Truecolor,
            _ => Self::Ansi256,
        }
    }
}

impl From<ColorLevel> for u8 {
    fn from(l: ColorLevel) -> Self {
        l as Self
    }
}

impl ColorLevel {
    /// The string label chalk uses (`ansi16` / `ansi256` / `truecolor`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None | Self::Ansi16 => "ansi16",
            Self::Ansi256 => "ansi256",
            Self::Truecolor => "truecolor",
        }
    }
}

/// A color the render layer can emit.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum Color {
    /// A default terminal color — no SGR override.
    #[default]
    Default,
    /// A named 16-color (`red`, `bright-green`, …). Names must match chalk.
    Named(String),
    /// An ANSI 256-color index.
    Ansi256(u8),
    /// A truecolor triple.
    Rgb { r: u8, g: u8, b: u8 },
}

/// Two-stop gradient. The render layer interpolates per-character in RGB.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    pub start: (u8, u8, u8),
    pub end: (u8, u8, u8),
}

impl Gradient {
    /// Linear interpolation between `start` and `end` at fraction `t` in
    /// `0.0..=1.0`. Clamps outside range.
    #[must_use]
    pub fn sample(&self, t: f32) -> (u8, u8, u8) {
        let clamped = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| -> u8 {
            let a_f = f32::from(a);
            let b_f = f32::from(b);
            (a_f + (b_f - a_f) * clamped).round().clamp(0.0, 255.0) as u8
        };
        (
            lerp(self.start.0, self.end.0),
            lerp(self.start.1, self.end.1),
            lerp(self.start.2, self.end.2),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_level_serde_round_trip() {
        for level in [
            ColorLevel::None,
            ColorLevel::Ansi16,
            ColorLevel::Ansi256,
            ColorLevel::Truecolor,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let back: ColorLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(level, back);
        }
    }

    #[test]
    fn color_level_deserializes_from_int() {
        let level: ColorLevel = serde_json::from_str("3").unwrap();
        assert_eq!(level, ColorLevel::Truecolor);
    }

    #[test]
    fn color_level_out_of_range_falls_back_to_ansi256() {
        let level: ColorLevel = serde_json::from_str("42").unwrap();
        assert_eq!(level, ColorLevel::Ansi256);
    }

    #[test]
    fn gradient_endpoints() {
        let g = Gradient {
            start: (0, 0, 0),
            end: (255, 255, 255),
        };
        assert_eq!(g.sample(0.0), (0, 0, 0));
        assert_eq!(g.sample(1.0), (255, 255, 255));
    }

    #[test]
    fn gradient_midpoint() {
        let g = Gradient {
            start: (0, 0, 0),
            end: (200, 100, 50),
        };
        assert_eq!(g.sample(0.5), (100, 50, 25));
    }

    #[test]
    fn gradient_clamps_out_of_range() {
        let g = Gradient {
            start: (10, 10, 10),
            end: (20, 20, 20),
        };
        assert_eq!(g.sample(-1.0), (10, 10, 10));
        assert_eq!(g.sample(2.0), (20, 20, 20));
    }
}
