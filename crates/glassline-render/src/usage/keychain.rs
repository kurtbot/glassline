// Parser fns are compiled on all platforms so unit tests run in CI, but
// they're only *called* from the macOS-only reader path — silence the
// dead-code warning on non-macOS targets.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

//! macOS Keychain credential resolution + `security dump-keychain` mdat parsing.
//!
//! On macOS Claude Code stores the OAuth token under service
//! `Claude Code-credentials` in the login keychain. We mirror TS
//! ccstatusline: read the password via `security find-generic-password
//! -w` (clean stdout) and read the mdat via a separate `security
//! dump-keychain` call parsed per-block. See
//! [[usage_hardening_design_v1.2]] §4.2 and T-2.2 of
//! [[usage_hardening_impl_plan_v1.0]].
//!
//! Parser fns (`split_on_keychain_boundary`, `parse_mdat_from_block`,
//! `normalize_security_timedate`, `decode_hex_ascii`) are compiled on
//! ALL platforms so unit tests run on Windows/Linux CI even though
//! `security(1)` is macOS-only.

pub(super) const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

/// Read the OAuth token AND its mdat from the macOS Keychain.
/// Returns `None` if either lookup fails. Called only when we need to
/// pick between the file path and the keychain source (both present).
#[cfg(target_os = "macos")]
pub(super) fn read_from_keychain_with_mdat() -> Option<(String, u64)> {
    let secret = read_keychain_secret()?;
    // mdat is best-effort — if we can't read it, treat as epoch so
    // a valid file mtime always wins over an unknown-age keychain.
    let mdat = read_keychain_mdat_ms().unwrap_or(0);
    Some((secret, mdat))
}

#[cfg(target_os = "macos")]
fn read_keychain_secret() -> Option<String> {
    let out = std::process::Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-w"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(target_os = "macos")]
fn read_keychain_mdat_ms() -> Option<u64> {
    let out = std::process::Command::new("security")
        .args(["dump-keychain"])
        .output()
        .ok()?;
    let dump = String::from_utf8_lossy(&out.stdout);
    let svce_needle = format!("\"svce\"<blob>=\"{KEYCHAIN_SERVICE}\"");
    for block in split_on_keychain_boundary(&dump) {
        if block.contains(&svce_needle)
            && let Some(ms) = parse_mdat_from_block(block)
        {
            return Some(ms);
        }
    }
    None
}

/// Split `security dump-keychain` output on the `^keychain:` boundary,
/// mirroring TS `.split(/(?=^keychain:\s)/m).filter(nonEmpty)`.
///
/// Each returned slice starts with `keychain:` unless the very first
/// byte of the dump isn't a `keychain:` line — in which case the
/// leading section is returned as its own block. Empty/whitespace-only
/// slices are filtered out.
pub(super) fn split_on_keychain_boundary(dump: &str) -> Vec<&str> {
    let mut boundaries: Vec<usize> = Vec::new();
    let mut pos: usize = 0;
    for line in dump.split_inclusive('\n') {
        if line.starts_with("keychain:") {
            boundaries.push(pos);
        }
        pos += line.len();
    }

    let mut result: Vec<&str> = Vec::new();
    if boundaries.is_empty() {
        if !dump.trim().is_empty() {
            result.push(dump);
        }
        return result;
    }
    if boundaries[0] > 0 {
        result.push(&dump[..boundaries[0]]);
    }
    for i in 0..boundaries.len() {
        let start = boundaries[i];
        let end = boundaries.get(i + 1).copied().unwrap_or(dump.len());
        result.push(&dump[start..end]);
    }
    result
        .into_iter()
        .filter(|b| !b.trim().is_empty())
        .collect()
}

/// Extract the `mdat` timedate from a `dump-keychain` block. Supports
/// both TS regex forms:
/// 1. `"mdat"<timedate>=(?:0x[0-9A-Fa-f]+\s+)?"YYYYMMDDHHMMSSZ"`
/// 2. `"mdat"<timedate>=0x[0-9A-Fa-f]+` (no quoted date; decode hex → ASCII date)
///
/// Form 1 wins if both are present on the same line.
pub(super) fn parse_mdat_from_block(block: &str) -> Option<u64> {
    for line in block.lines() {
        let Some(rest) = line.trim_start().strip_prefix("\"mdat\"<timedate>=") else {
            continue;
        };

        // Form 1: quoted date, possibly preceded by hex.
        if let Some(quote_start) = rest.find('"') {
            let inner = &rest[quote_start + 1..];
            if let Some(quote_end) = inner.find('"')
                && let Some(ms) = normalize_security_timedate(&inner[..quote_end])
            {
                return Some(ms);
            }
        }

        // Form 2: hex-only. Decode ASCII date string.
        if let Some(after_prefix) = rest.strip_prefix("0x") {
            let hex_end = after_prefix
                .find(|c: char| !c.is_ascii_hexdigit())
                .unwrap_or(after_prefix.len());
            let hex = &after_prefix[..hex_end];
            if let Some(decoded) = decode_hex_ascii(hex)
                && let Some(ms) = normalize_security_timedate(&decoded)
            {
                return Some(ms);
            }
        }
    }
    None
}

/// Decode an even-length ASCII-hex string as UTF-8 bytes. TS uses this
/// to decode the `0x…` form of the mdat field back into a
/// `"YYYYMMDDHHMMSSZ"` string.
pub(super) fn decode_hex_ascii(hex: &str) -> Option<String> {
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks(2) {
        let s = std::str::from_utf8(chunk).ok()?;
        let b = u8::from_str_radix(s, 16).ok()?;
        bytes.push(b);
    }
    String::from_utf8(bytes).ok()
}

/// Parse a `security(1)` timedate string (`YYYYMMDDHHMMSSZ`, 15 chars,
/// UTC) into unix-milliseconds.
pub(super) fn normalize_security_timedate(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.len() != 15 || !raw.ends_with('Z') {
        return None;
    }
    let year: i32 = raw[0..4].parse().ok()?;
    let month: u8 = raw[4..6].parse().ok()?;
    let day: u8 = raw[6..8].parse().ok()?;
    let hour: u8 = raw[8..10].parse().ok()?;
    let minute: u8 = raw[10..12].parse().ok()?;
    let second: u8 = raw[12..14].parse().ok()?;
    let month = time::Month::try_from(month).ok()?;
    let date = time::Date::from_calendar_date(year, month, day).ok()?;
    let t = time::Time::from_hms(hour, minute, second).ok()?;
    let dt = time::PrimitiveDateTime::new(date, t).assume_utc();
    let nanos = dt.unix_timestamp_nanos();
    if nanos < 0 {
        return None;
    }
    Some((nanos / 1_000_000) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_single_block_without_prefix() {
        let dump = "some header\nno keychain marker here\n";
        let blocks = split_on_keychain_boundary(dump);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], dump);
    }

    #[test]
    fn split_multiple_blocks() {
        let dump = concat!(
            "keychain: \"/Library/Keychains/login.keychain-db\"\n",
            "class: genp\n",
            "attributes: foo\n",
            "keychain: \"/Library/Keychains/system.keychain\"\n",
            "class: genp\n",
        );
        let blocks = split_on_keychain_boundary(dump);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].starts_with("keychain: \"/Library/Keychains/login"));
        assert!(blocks[1].starts_with("keychain: \"/Library/Keychains/system"));
    }

    #[test]
    fn split_empty_dump() {
        assert!(split_on_keychain_boundary("").is_empty());
        assert!(split_on_keychain_boundary("   \n\t\n").is_empty());
    }

    #[test]
    fn split_prefix_then_boundary() {
        let dump = concat!("preface line\n", "keychain: \"/x\"\n", "class: genp\n",);
        let blocks = split_on_keychain_boundary(dump);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].starts_with("preface"));
        assert!(blocks[1].starts_with("keychain:"));
    }

    #[test]
    fn mdat_quoted_date_form() {
        let block = concat!(
            "keychain: \"/x\"\n",
            "class: genp\n",
            "    \"mdat\"<timedate>=\"20260819130800Z\"\n",
        );
        let ms = parse_mdat_from_block(block).expect("mdat");
        // 2026-08-19T13:08:00Z
        assert!(ms > 1_780_000_000_000, "got {ms}");
    }

    #[test]
    fn mdat_hex_only_form() {
        // ASCII "20260819130800Z" → hex 3230323630383139313330383030 5a
        let hex_body = "20260819130800Z"
            .bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let block = format!("keychain: \"/x\"\n    \"mdat\"<timedate>=0x{hex_body}\n");
        let ms = parse_mdat_from_block(&block).expect("mdat");
        assert!(ms > 1_780_000_000_000, "got {ms}");
    }

    #[test]
    fn mdat_hex_preceded_quoted_prefers_quoted() {
        // Real output sometimes has both: hex first, then quoted.
        let block = concat!(
            "keychain: \"/x\"\n",
            "    \"mdat\"<timedate>=0xdeadbeef  \"20260819130800Z\"\n",
        );
        let ms = parse_mdat_from_block(block).expect("mdat (quoted wins)");
        assert!(ms > 1_780_000_000_000, "got {ms}");
    }

    #[test]
    fn mdat_missing_field_returns_none() {
        let block = "keychain: \"/x\"\nclass: genp\nno mdat here\n";
        assert!(parse_mdat_from_block(block).is_none());
    }

    #[test]
    fn mdat_malformed_date_returns_none() {
        let block = "\"mdat\"<timedate>=\"ZZ260819130800Z\"\n";
        assert!(parse_mdat_from_block(block).is_none());
    }

    #[test]
    fn normalize_timedate_valid() {
        // 1970-01-01T00:00:00Z → 0 ms
        assert_eq!(normalize_security_timedate("19700101000000Z"), Some(0));
    }

    #[test]
    fn normalize_timedate_length_mismatch() {
        assert!(normalize_security_timedate("2026081913080Z").is_none());
        assert!(normalize_security_timedate("202608191308000Z").is_none());
    }

    #[test]
    fn normalize_timedate_missing_z() {
        assert!(normalize_security_timedate("20260819130800A").is_none());
    }

    #[test]
    fn normalize_timedate_bad_month() {
        assert!(normalize_security_timedate("20261319130800Z").is_none());
    }

    #[test]
    fn decode_hex_ascii_valid() {
        assert_eq!(decode_hex_ascii("48656c6c6f"), Some("Hello".to_string()));
    }

    #[test]
    fn decode_hex_ascii_odd_length_returns_none() {
        assert!(decode_hex_ascii("48656c6c6").is_none());
    }

    #[test]
    fn decode_hex_ascii_non_hex_returns_none() {
        assert!(decode_hex_ascii("ZZ").is_none());
    }
}
