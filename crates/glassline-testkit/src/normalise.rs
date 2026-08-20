//! ANSI-to-`AttrRuns` normaliser (design §4.10).
//!
//! Parses SGR sequences (`\x1b[...m`) into a running style, walks the visible
//! characters, and emits one [`AttrRun`] per contiguous character range with a
//! stable style. Different SGR encodings that produce the same on-screen
//! result normalise to the same [`AttrRuns`].
//!
//! Non-SGR ANSI (cursor moves, screen-clear, etc) is stripped; hyperlink
//! escape wrappers (`\x1b]8;...\x1b\\`) are consumed with their targets
//! preserved on the resulting run.

use std::collections::BTreeSet;

/// Distinct style flags that can appear on a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttrFlag {
    Bold,
    Dim,
    Italic,
    Underline,
    Reverse,
}

/// The set of flags active for a run.
pub type AttrSet = BTreeSet<AttrFlag>;

/// A single normalised run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrRun {
    pub text: String,
    pub fg: FgBg,
    pub bg: FgBg,
    pub flags: AttrSet,
    pub hyperlink: Option<String>,
}

/// Color repr as it appears normalised. `Default` == no SGR override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FgBg {
    Default,
    Ansi16(u8),
    Ansi256(u8),
    Rgb(u8, u8, u8),
}

/// Full normalised output for one blob.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AttrRuns {
    pub runs: Vec<AttrRun>,
}

impl AttrRuns {
    /// Concatenate every run's text — this is the "visible string".
    #[must_use]
    pub fn visible(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// Human-diffable YAML rendering (used by the parity assertion panic).
    #[must_use]
    pub fn debug_yaml(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("visible: {:?}\n", self.visible()));
        out.push_str("runs:\n");
        for run in &self.runs {
            out.push_str(&format!(
                "  - text: {:?}\n    fg: {:?}\n    bg: {:?}\n    flags: {:?}\n",
                run.text, run.fg, run.bg, run.flags,
            ));
            if let Some(link) = &run.hyperlink {
                out.push_str(&format!("    hyperlink: {link:?}\n"));
            }
        }
        out
    }
}

/// Parse a raw ANSI-bearing string into a normalised [`AttrRuns`].
#[must_use]
pub fn normalise(input: &str) -> AttrRuns {
    let mut state = Style::default();
    let mut runs: Vec<AttrRun> = Vec::new();
    let mut buf = String::new();

    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == 0x1b && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'[' => {
                    // CSI ... final byte. Only SGR ('m') mutates the style;
                    // everything else is dropped.
                    let (params, end) = read_csi(bytes, i + 2);
                    if bytes.get(end).copied() == Some(b'm') {
                        // Flush before applying style change.
                        flush_buf(&mut buf, &state, &mut runs);
                        apply_sgr(&params, &mut state);
                    }
                    i = end + 1;
                    continue;
                }
                b']' => {
                    // OSC — we're interested in `]8;params;target ST`, the
                    // hyperlink protocol.
                    if let Some((link, end)) = read_osc_hyperlink(bytes, i + 2) {
                        // Empty target = close the hyperlink; otherwise open.
                        flush_buf(&mut buf, &state, &mut runs);
                        state.hyperlink = if link.is_empty() { None } else { Some(link) };
                        i = end;
                        continue;
                    }
                    // Any other OSC — skip until ST or BEL.
                    let end = skip_osc(bytes, i + 2);
                    i = end;
                    continue;
                }
                _ => {
                    // Other escapes — swallow just the two bytes.
                    i += 2;
                    continue;
                }
            }
        }
        // Non-ANSI byte — accumulate onto the current run.
        buf.push(b as char);
        i += 1;
    }
    flush_buf(&mut buf, &state, &mut runs);
    AttrRuns { runs }
}

fn flush_buf(buf: &mut String, state: &Style, runs: &mut Vec<AttrRun>) {
    if buf.is_empty() {
        return;
    }
    // Merge adjacent identical styles for a stable normalisation.
    if let Some(last) = runs.last_mut()
        && last.fg == state.fg
        && last.bg == state.bg
        && last.flags == state.flags
        && last.hyperlink == state.hyperlink
    {
        last.text.push_str(buf);
        buf.clear();
        return;
    }
    runs.push(AttrRun {
        text: std::mem::take(buf),
        fg: state.fg.clone(),
        bg: state.bg.clone(),
        flags: state.flags.clone(),
        hyperlink: state.hyperlink.clone(),
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Style {
    fg: FgBg,
    bg: FgBg,
    flags: AttrSet,
    hyperlink: Option<String>,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: FgBg::Default,
            bg: FgBg::Default,
            flags: AttrSet::new(),
            hyperlink: None,
        }
    }
}

/// Read the numeric-plus-`;` params of a CSI sequence starting at `start`.
/// Returns the raw params slice + the index of the final byte.
fn read_csi(bytes: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut params = Vec::new();
    let mut i = start;
    while i < bytes.len() {
        let b = bytes[i];
        // CSI final bytes are 0x40..=0x7e; parameters are 0x30..=0x3f and
        // intermediates are 0x20..=0x2f. We collect the params and stop at
        // the first final byte.
        if (0x40..=0x7e).contains(&b) {
            return (params, i);
        }
        params.push(b);
        i += 1;
    }
    (params, i)
}

/// Skip a generic OSC sequence up to the string terminator (`ESC \` or BEL).
fn skip_osc(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == 0x07 {
            return i + 1;
        }
        if bytes[i] == 0x1b && bytes.get(i + 1).copied() == Some(b'\\') {
            return i + 2;
        }
        i += 1;
    }
    i
}

/// Try to read an `]8;params;target` OSC (the hyperlink protocol).
fn read_osc_hyperlink(bytes: &[u8], start: usize) -> Option<(String, usize)> {
    if bytes.get(start).copied() != Some(b'8') || bytes.get(start + 1).copied() != Some(b';') {
        return None;
    }
    // We're inside `]8;<params>;<target><ST>`. Skip past the params section.
    let mut i = start + 2;
    while i < bytes.len() && bytes[i] != b';' {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let target_start = i + 1;
    let mut j = target_start;
    while j < bytes.len() {
        if bytes[j] == 0x07 {
            let target = String::from_utf8_lossy(&bytes[target_start..j]).into_owned();
            return Some((target, j + 1));
        }
        if bytes[j] == 0x1b && bytes.get(j + 1).copied() == Some(b'\\') {
            let target = String::from_utf8_lossy(&bytes[target_start..j]).into_owned();
            return Some((target, j + 2));
        }
        j += 1;
    }
    None
}

/// Apply an SGR parameter run to `state`.
fn apply_sgr(params: &[u8], state: &mut Style) {
    let text = std::str::from_utf8(params).unwrap_or("");
    let ints: Vec<i32> = if text.is_empty() {
        vec![0]
    } else {
        text.split(';')
            .map(|s| s.parse::<i32>().unwrap_or(0))
            .collect()
    };
    let mut i = 0;
    while i < ints.len() {
        let code = ints[i];
        match code {
            0 => *state = Style::default(),
            1 => {
                state.flags.insert(AttrFlag::Bold);
            }
            2 => {
                state.flags.insert(AttrFlag::Dim);
            }
            3 => {
                state.flags.insert(AttrFlag::Italic);
            }
            4 => {
                state.flags.insert(AttrFlag::Underline);
            }
            7 => {
                state.flags.insert(AttrFlag::Reverse);
            }
            22 => {
                state.flags.remove(&AttrFlag::Bold);
                state.flags.remove(&AttrFlag::Dim);
            }
            23 => {
                state.flags.remove(&AttrFlag::Italic);
            }
            24 => {
                state.flags.remove(&AttrFlag::Underline);
            }
            27 => {
                state.flags.remove(&AttrFlag::Reverse);
            }
            30..=37 => state.fg = FgBg::Ansi16((code - 30) as u8),
            39 => state.fg = FgBg::Default,
            40..=47 => state.bg = FgBg::Ansi16((code - 40) as u8),
            49 => state.bg = FgBg::Default,
            90..=97 => state.fg = FgBg::Ansi16((code - 90 + 8) as u8),
            100..=107 => state.bg = FgBg::Ansi16((code - 100 + 8) as u8),
            38 | 48 => {
                let is_fg = code == 38;
                if let Some(&5) = ints.get(i + 1) {
                    if let Some(&idx) = ints.get(i + 2) {
                        let val = FgBg::Ansi256(idx.clamp(0, 255) as u8);
                        if is_fg {
                            state.fg = val;
                        } else {
                            state.bg = val;
                        }
                    }
                    i += 2;
                } else if let Some(&2) = ints.get(i + 1) {
                    let r = ints.get(i + 2).copied().unwrap_or(0).clamp(0, 255) as u8;
                    let g = ints.get(i + 3).copied().unwrap_or(0).clamp(0, 255) as u8;
                    let b = ints.get(i + 4).copied().unwrap_or(0).clamp(0, 255) as u8;
                    let val = FgBg::Rgb(r, g, b);
                    if is_fg {
                        state.fg = val;
                    } else {
                        state.bg = val;
                    }
                    i += 4;
                }
            }
            _ => {}
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_normalises_to_single_default_run() {
        let out = normalise("hello");
        assert_eq!(out.runs.len(), 1);
        assert_eq!(out.runs[0].text, "hello");
        assert_eq!(out.runs[0].fg, FgBg::Default);
        assert!(out.runs[0].flags.is_empty());
    }

    #[test]
    fn different_sgr_encodings_are_equivalent() {
        let combined = "\x1b[1;32mhi\x1b[0m";
        let split = "\x1b[1m\x1b[32mhi\x1b[0m";
        assert_eq!(normalise(combined), normalise(split));
    }

    #[test]
    fn bg_color_captured() {
        let out = normalise("\x1b[41mx\x1b[0m");
        assert_eq!(out.runs[0].bg, FgBg::Ansi16(1));
    }

    #[test]
    fn truecolor_fg_captured() {
        let out = normalise("\x1b[38;2;10;20;30my\x1b[0m");
        assert_eq!(out.runs[0].fg, FgBg::Rgb(10, 20, 30));
    }

    #[test]
    fn ansi256_bg_captured() {
        let out = normalise("\x1b[48;5;123mz\x1b[0m");
        assert_eq!(out.runs[0].bg, FgBg::Ansi256(123));
    }

    #[test]
    fn bold_and_underline_survive_split_reset() {
        let out = normalise("\x1b[1mBOLD\x1b[22mplain\x1b[4mUNDER\x1b[0m");
        assert_eq!(out.runs[0].text, "BOLD");
        assert!(out.runs[0].flags.contains(&AttrFlag::Bold));
        assert_eq!(out.runs[1].text, "plain");
        assert!(out.runs[1].flags.is_empty());
        assert_eq!(out.runs[2].text, "UNDER");
        assert!(out.runs[2].flags.contains(&AttrFlag::Underline));
    }

    #[test]
    fn hyperlink_wraps_a_run() {
        let out = normalise("\x1b]8;;https://example.com\x07link\x1b]8;;\x07");
        assert_eq!(out.runs.len(), 1);
        assert_eq!(out.runs[0].text, "link");
        assert_eq!(
            out.runs[0].hyperlink.as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn non_sgr_csi_is_stripped() {
        // `\x1b[2K` = erase-line; irrelevant to render output.
        let out = normalise("before\x1b[2Kafter");
        assert_eq!(out.runs[0].text, "beforeafter");
    }

    #[test]
    fn empty_input_gives_no_runs() {
        assert!(normalise("").runs.is_empty());
    }

    #[test]
    fn assert_visually_equivalent_self_check() {
        crate::assert_visually_equivalent("\x1b[1;32mhi\x1b[0m", "\x1b[1m\x1b[32mhi\x1b[0m");
    }
}
