//! Slurp stdin into a `Vec<u8>`.
//!
//! Kept in its own module so the pipeline layer can be unit-tested with a
//! synthetic byte slice while the real binary path uses [`slurp_stdin`].

use std::io::{self, Read};

/// Read stdin to EOF. On success returns the raw bytes as a `String` (UTF-8
/// lossy — Claude Code always sends JSON, but a stray byte should degrade
/// gracefully rather than crash).
pub fn slurp_stdin() -> io::Result<String> {
    let mut buf = Vec::with_capacity(4096);
    io::stdin().read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}
