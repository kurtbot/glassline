//! Credential resolution for the Anthropic OAuth usage endpoint.
//!
//! Default source: `$CLAUDE_CONFIG_DIR/.credentials.json` (or
//! `~/.claude/.credentials.json`). On macOS the resolver also checks
//! the `Claude Code-credentials` Keychain entry and returns whichever
//! source has the newer modification time. See

use std::{fs, path::PathBuf, time::UNIX_EPOCH};

use serde::Deserialize;
use thiserror::Error;

pub(super) fn resolve_access_token() -> Result<String, ReadTokenError> {
    let file_pair = read_from_file_with_mtime();

    #[cfg(target_os = "macos")]
    let kc_pair = super::keychain::read_from_keychain_with_mdat();

    #[cfg(not(target_os = "macos"))]
    let kc_pair: Option<(String, u64)> = None;

    match (file_pair, kc_pair) {
        (Some((f_tok, f_mtime)), Some((k_tok, k_mtime))) => {
            Ok(if k_mtime > f_mtime { k_tok } else { f_tok })
        }
        (Some((tok, _)), None) => Ok(tok),
        (None, Some((tok, _))) => Ok(tok),
        (None, None) => Err(ReadTokenError::NoFile),
    }
}

fn credentials_file_path() -> Option<PathBuf> {
    let dir = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(|h| PathBuf::from(h).join(".claude"))
        })?;
    Some(dir.join(".credentials.json"))
}

fn read_from_file_with_mtime() -> Option<(String, u64)> {
    let path = credentials_file_path()?;
    let meta = fs::metadata(&path).ok()?;
    let mtime_ms = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let raw = fs::read_to_string(&path).ok()?;
    let parsed: CredentialsFile = serde_json::from_str(&raw).ok()?;
    let token = parsed.claude_ai_oauth.and_then(|c| c.access_token)?;
    Some((token, mtime_ms))
}

#[derive(Debug, Error)]
pub(super) enum ReadTokenError {
    #[error("credentials file not found")]
    NoFile,
    #[error("credentials JSON malformed")]
    #[allow(dead_code)]
    BadJson,
    #[error("no access token in credentials")]
    #[allow(dead_code)]
    NoToken,
}

#[derive(Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<ClaudeAiOauth>,
}

#[derive(Deserialize)]
struct ClaudeAiOauth {
    #[serde(rename = "accessToken")]
    access_token: Option<String>,
}
