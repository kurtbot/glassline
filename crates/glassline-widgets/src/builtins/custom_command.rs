//! `custom-command` — runs an arbitrary user-supplied command and
//! renders its stdout. In-crate stopgap for the deferred
//! `glassline-ext` external-widget loader. Port of upstream
//! `CustomCommand.tsx`.
//!
//! # Configuration
//!
//! - `spec.command` — required. The binary path (or bare command name
//!   found on PATH). NOT interpreted by a shell — direct exec.
//! - `spec.args` — argv slice. Optional.
//! - `spec.timeout_ms` — process kill deadline. Default 5000 ms.
//! - `spec.cache_ttl_ms` — on-disk cache TTL. Default 0 (no persistent
//!   cache; every render invokes the command). When >0, output is
//!   cached in `<cache-dir>/glassline/custom-cmd-cache.json`.
//!
//! # Security posture
//!
//! `spec.command` runs with the user's own privileges via direct exec
//! (no `sh -c`). The `settings.json` file is user-owned, so a malicious
//! command in there would already imply the attacker had write access
//! to the user's home directory — no additional attack surface.
//!
//! Stderr is discarded. Non-zero exit → empty output.

use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use glassline_core::{
    render_context::RenderContext,
    settings::WidgetSpec,
    span::StyledSpan,
    widget::{Widget, WidgetRequirements},
};
use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use crate::common::styled;

const DEFAULT_TIMEOUT_MS: u32 = 5_000;
const CACHE_FILENAME: &str = "custom-cmd-cache.json";

pub fn factory() -> Box<dyn Widget> {
    Box::new(CustomCommand)
}

pub struct CustomCommand;

impl Widget for CustomCommand {
    fn id(&self) -> &'static str {
        "custom-command"
    }
    fn requirements(&self) -> WidgetRequirements {
        WidgetRequirements::NONE
    }

    fn render(&self, spec: &WidgetSpec, _ctx: &RenderContext) -> Vec<StyledSpan> {
        let Some(command) = spec.command.as_deref().filter(|s| !s.is_empty()) else {
            return Vec::new();
        };
        let args: Vec<String> = spec.args.clone().unwrap_or_default();
        let timeout_ms = spec.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let cache_ttl_ms = spec.cache_ttl_ms.unwrap_or(0);
        let now_ms = now_ms();
        let key = cache_key(command, &args);

        // In-process cache hit — same widget instance called twice in
        // one render pass returns the same output without re-spawning.
        if let Some(cached) = process_cache_get(&key) {
            return styled(spec, cached);
        }

        // Disk cache (opt-in via cache_ttl_ms > 0).
        if cache_ttl_ms > 0
            && let Some(entry) = disk_cache_read(&key)
            && now_ms.saturating_sub(entry.stamped_at_ms) < u64::from(cache_ttl_ms)
        {
            process_cache_put(&key, &entry.stdout);
            return styled(spec, entry.stdout);
        }

        // Actually run the command.
        let stdout = run_command(command, &args, timeout_ms);
        if stdout.is_empty() {
            return Vec::new();
        }

        if cache_ttl_ms > 0 {
            disk_cache_write(&key, now_ms, &stdout);
        }
        process_cache_put(&key, &stdout);
        styled(spec, stdout)
    }
}

// ---------- subprocess ----------

fn run_command(command: &str, args: &[String], timeout_ms: u32) -> String {
    let child = Command::new(command)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return String::new();
    };
    let timeout = Duration::from_millis(u64::from(timeout_ms));
    let status = match child.wait_timeout(timeout) {
        Ok(Some(status)) => status,
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            return String::new();
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return String::new();
        }
    };
    if !status.success() {
        return String::new();
    }
    // wait_timeout returned Some — the stdout pipe is fully drained by
    // the reaped child. Read it now.
    let mut stdout = child.stdout.take();
    if let Some(handle) = stdout.as_mut() {
        let mut buf = String::new();
        if std::io::Read::read_to_string(handle, &mut buf).is_ok() {
            return buf.trim().to_string();
        }
    }
    String::new()
}

// ---------- caching ----------

fn cache_key(command: &str, args: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    command.hash(&mut hasher);
    args.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskCacheEntry {
    stamped_at_ms: u64,
    stdout: String,
}

fn cache_path() -> Option<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|h| PathBuf::from(h).join("AppData").join("Local"))
            })?
    } else {
        std::env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?
    };
    Some(base.join("glassline").join(CACHE_FILENAME))
}

fn disk_cache_read(key: &str) -> Option<DiskCacheEntry> {
    let path = cache_path()?;
    let raw = fs::read_to_string(&path).ok()?;
    let map: HashMap<String, DiskCacheEntry> = serde_json::from_str(&raw).ok()?;
    map.get(key).cloned()
}

fn disk_cache_write(key: &str, now_ms: u64, stdout: &str) {
    let Some(path) = cache_path() else { return };
    let Some(parent) = path.parent() else { return };
    let _ = fs::create_dir_all(parent);
    // Merge into existing file so unrelated cache entries aren't clobbered.
    let mut map: HashMap<String, DiskCacheEntry> = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    map.insert(
        key.to_string(),
        DiskCacheEntry {
            stamped_at_ms: now_ms,
            stdout: stdout.to_string(),
        },
    );
    if let Ok(bytes) = serde_json::to_vec(&map) {
        let _ = fs::write(path, bytes);
    }
}

// ---------- per-invocation cache ----------

static PROCESS_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn process_cache_get(key: &str) -> Option<String> {
    let cache = PROCESS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache.lock().ok()?.get(key).cloned()
}

fn process_cache_put(key: &str, value: &str) {
    let cache = PROCESS_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut g) = cache.lock() {
        g.insert(key.to_string(), value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_command_missing() {
        let spec = WidgetSpec::new("1", "custom-command");
        let spans = CustomCommand.render(&spec, &RenderContext::default());
        assert!(spans.is_empty());
    }

    #[test]
    fn empty_when_command_is_empty_string() {
        let mut spec = WidgetSpec::new("1", "custom-command");
        spec.command = Some("".to_string());
        let spans = CustomCommand.render(&spec, &RenderContext::default());
        assert!(spans.is_empty());
    }

    #[test]
    fn cache_key_stable_for_same_input() {
        let a = cache_key("echo", &["hello".to_string()]);
        let b = cache_key("echo", &["hello".to_string()]);
        assert_eq!(a, b);
    }

    #[test]
    fn cache_key_differs_for_different_args() {
        let a = cache_key("echo", &["hello".to_string()]);
        let b = cache_key("echo", &["world".to_string()]);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_key_differs_for_different_command() {
        let a = cache_key("echo", &[]);
        let b = cache_key("cat", &[]);
        assert_ne!(a, b);
    }

    #[test]
    fn runs_bogus_command_and_returns_empty() {
        // Not testing the actual widget render (uses PROCESS_CACHE which
        // would leak state across tests); just exercise the subprocess
        // path with a definitely-nonexistent binary.
        let out = run_command("C:/nonexistent/no-such-binary-glassline-test", &[], 100);
        assert!(out.is_empty());
    }
}
