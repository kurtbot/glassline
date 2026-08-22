// SPDX-FileCopyrightText: 2026 Kurt Milan
// SPDX-License-Identifier: MIT

//! `glassline` — hot-path binary entry point.
//!
//! Argv handled by a hand-rolled parser (clap stays out of the hot path per
//! design §4.1). Subcommands:
//!   * `install`   / `uninstall`   — wire glassline into Claude Code
//!   * `import`                    — one-shot migrate ccstatusline → glassline
//!   * `demo`                      — preview an animation without piping stdin
//!   * `--version` / `-V`          — print version + exit
//!   * `--help`    / `-h`          — usage
//!   * `--config <path>`           — override settings.json path
//!
//! TTY shim: bare `glassline` in a terminal (no stdin, no args) exec's
//! `glassline-tui` if it's installed next to the render binary. Piped
//! stdin (Claude Code invoking us) proceeds to render mode normally.
//!
//! Render mode (no subcommand): slurp stdin, load config, render pipeline,
//! write to stdout. On the P1 slice a **first-run** load falls back to a
//! small marker widget so the user sees glassline is alive even before they
//! customise anything. Once real MVP widgets ship (T-1.7+), the first-run
//! defaults will produce a useful line on their own.

use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
    process::{Command, ExitCode},
};

use glassline_core::{
    color::Color,
    render_context::{BlockMetrics, RenderContext},
    settings::{Settings, WidgetSpec},
    span::StyledSpan,
    status_json::StatusJson,
    widget::WidgetRequirements,
};
use glassline_render::{
    adapter::{REGISTRY as ADAPTER_REGISTRY, REGISTRY_ORDER as ADAPTER_ORDER},
    ansi::spans_to_string,
    config::{LoadOutcome, default_settings_path, load},
    import::{self, ImportOpts},
    install::{InstallOpts, Scope, render_report},
    pipeline::{compute_requirements, render_to_string},
    render_cache,
    stdin_reader::slurp_stdin,
    transcript, usage,
};

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();

    if raw_args.iter().any(|a| a == "--version" || a == "-V") {
        println!("glassline {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // Route subcommand FIRST so `glassline demo --help` doesn't hit the
    // top-level help; render-mode --help still works because that path
    // has no subcommand keyword.
    match raw_args.first().map(String::as_str) {
        Some("install") => run_install_cmd(&raw_args[1..]),
        Some("uninstall") => run_uninstall_cmd(&raw_args[1..]),
        Some("import") => run_import_cmd(&raw_args[1..]),
        Some("demo") => glassline_render::demo::run(&raw_args[1..]),
        _ => {
            if raw_args.iter().any(|a| a == "--help" || a == "-h") {
                print_help();
                return ExitCode::SUCCESS;
            }
            // TTY shim: bare `glassline` in a terminal with no piped
            // stdin means the user typed the command by hand — send
            // them to the editor. Piped input (Claude Code invoking us)
            // proceeds to render_mode normally.
            if raw_args.is_empty() && std::io::stdin().is_terminal() {
                return exec_tui();
            }
            run_render(&raw_args)
        }
    }
}

/// Spawn the sibling `glassline-tui` binary and forward its exit code.
/// Skipping the shim (falling through to render mode) is the fallback
/// when the editor isn't installed — the user sees the usual "no
/// stdin" message and can install it separately.
fn exec_tui() -> ExitCode {
    let Ok(exe) = std::env::current_exe() else {
        return run_render(&[]);
    };
    let Some(dir) = exe.parent() else {
        return run_render(&[]);
    };
    let editor = dir.join(if cfg!(windows) {
        "glassline-tui.exe"
    } else {
        "glassline-tui"
    });
    if !editor.exists() {
        // Editor not installed — fall through so the user still sees
        // a helpful message rather than a "not found" spawn error.
        eprintln!(
            "glassline: editor binary not found at {}\nInstall it with `cargo install --path crates/glassline-tui` or download from the release page.",
            editor.display()
        );
        return run_render(&[]);
    }
    let status = Command::new(&editor).status();
    match status {
        Ok(s) => s
            .code()
            .and_then(|c| u8::try_from(c).ok())
            .map(ExitCode::from)
            .unwrap_or(ExitCode::SUCCESS),
        Err(e) => {
            eprintln!("glassline: failed to spawn {}: {e}", editor.display());
            ExitCode::FAILURE
        }
    }
}

fn run_render(args: &[String]) -> ExitCode {
    let debug_enabled = std::env::var_os("GLASSLINE_DEBUG").is_some();
    let config_override = extract_config_override(args);

    let raw = match slurp_stdin() {
        Ok(s) => s,
        Err(e) => {
            debug_log(debug_enabled, "stdin-error", &e.to_string());
            println!("[glassline v{} stdin-err]", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
    };
    debug_log(debug_enabled, "stdin-bytes", &raw);

    if raw.trim().is_empty() {
        println!("[glassline v{} (no stdin)]", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }

    // T8 render-cache: build the key from stdin bytes + settings.json
    // mtime + a time-window quantum. If we've rendered identical inputs
    // within the TTL window (default 150ms), replay the cached output and
    // skip transcript scan / git shell-outs / usage probe / pipeline.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let settings_path_for_key = config_override
        .clone()
        .or_else(|| default_settings_path().ok());
    let cache_key = render_cache::build_key(
        raw.as_bytes(),
        settings_path_for_key.as_deref(),
        now_ms,
        render_cache::ttl_ms(),
    );
    if let Some(cached) = render_cache::try_read(&cache_key, now_ms) {
        debug_log(debug_enabled, "cache-hit", &cached);
        render_cache::record_stat(true, now_ms);
        println!("{cached}");
        return ExitCode::SUCCESS;
    }
    render_cache::record_stat(false, now_ms);

    let payload: StatusJson = match serde_json::from_str(&raw) {
        Ok(p) => p,
        Err(e) => {
            debug_log(debug_enabled, "parse-error", &e.to_string());
            println!("[glassline v{} parse-err]", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
    };

    let loaded = match load(config_override.as_deref()) {
        Ok(l) => l,
        Err(e) => {
            debug_log(debug_enabled, "config-error", &e.to_string());
            println!("[glassline v{} config-err]", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
    };
    debug_log(
        debug_enabled,
        "config-outcome",
        &format!("{:?} @ {}", loaded.outcome, loaded.path.display()),
    );

    // On first run swap in a friendly marker so the P1 slice shows *something*
    // even before the MVP widgets ship. Once real widgets land the loaded
    // defaults render fine on their own; drop this branch then.
    let settings = match loaded.outcome {
        LoadOutcome::FirstRun => first_run_slice_settings(),
        _ => loaded.settings,
    };

    // Build the render context, then conditionally prefill transcript
    // metrics based on which widgets on the visible lines actually need
    // them (mirror of TS `hasSpeedItems` / `hasCompactionWidget` scan).
    let requirements = compute_requirements(&settings);
    let mut ctx = RenderContext {
        data: Some(payload.clone()),
        now_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0),
        ..RenderContext::default()
    };

    let transcript_bits = WidgetRequirements::TRANSCRIPT
        | WidgetRequirements::COMPACTION
        | WidgetRequirements::SESSION_CLOCK
        | WidgetRequirements::SPEED
        | WidgetRequirements::CACHE;
    if let Some(transcript_path) = payload.transcript_path.as_deref()
        && (requirements.contains(WidgetRequirements::TRANSCRIPT)
            || requirements.contains(WidgetRequirements::COMPACTION)
            || requirements.contains(WidgetRequirements::SESSION_CLOCK)
            || requirements.contains(WidgetRequirements::SPEED)
            || requirements.contains(WidgetRequirements::CACHE))
    {
        let path = std::path::Path::new(transcript_path);
        match transcript::scan(path, requirements & transcript_bits) {
            Ok(scan) => {
                debug_log(
                    debug_enabled,
                    "transcript-scan",
                    &format!(
                        "input={} output={} ctx_len={} compact_count={} dur={:?} speed_dur_ms={}",
                        scan.tokens.input,
                        scan.tokens.output,
                        scan.tokens.context_length,
                        scan.compaction.count,
                        scan.session_duration,
                        scan.speed.total_duration_ms,
                    ),
                );
                ctx.token_metrics = Some(scan.tokens);
                ctx.compaction_data = Some(scan.compaction);
                ctx.session_duration = scan.session_duration;
                ctx.speed_metrics = Some(scan.speed);
                ctx.cache_timer = Some(glassline_core::render_context::CacheTimerState {
                    working: scan.cache_working,
                    last_touch_ms: scan.cache_last_touch_ms,
                });
            }
            Err(e) => {
                debug_log(debug_enabled, "transcript-error", &e.to_string());
            }
        }
    }

    if requirements.contains(WidgetRequirements::USAGE)
        && let Some(usage_data) = usage::fetch_or_cached()
    {
        debug_log(
            debug_enabled,
            "usage",
            &format!(
                "session={:?} weekly={:?} sonnet={:?} err={:?}",
                usage_data.session_usage,
                usage_data.weekly_usage,
                usage_data.weekly_sonnet_usage,
                usage_data.error,
            ),
        );
        // Populate BlockMetrics from the 5-hour bucket's reset stamp
        // so `block-reset-timer` / `block-timer` widgets render. Only
        // `resets_at` is needed for the timer countdown; `block_id`
        // and `started_at` remain None until Claude Code ships them.
        if let Some(resets_at) = usage_data.session_reset_at.clone() {
            ctx.block_metrics = Some(BlockMetrics {
                resets_at: Some(resets_at),
                ..Default::default()
            });
        }
        ctx.usage_data = Some(usage_data);
    }

    let output = match render_to_string(ctx, &settings) {
        Ok(s) => s,
        Err(e) => {
            debug_log(debug_enabled, "render-error", &e.to_string());
            println!("[glassline v{} render-err]", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
    };

    // Corrupt-file recovery: prepend a visible warning span so the user
    // notices without losing their status line.
    let mut composite = String::new();
    if let LoadOutcome::CorruptFallback { ref reason } = loaded.outcome {
        let warn = warning_banner(&format!("bad settings: {reason}"));
        composite.push_str(&spans_to_string(&[warn]));
        if !output.is_empty() {
            composite.push('\n');
        }
    }
    composite.push_str(&output);

    // Apply Claude Code UI tweaks (reset + nbsp) so per-widget colors
    // aren't clobbered by Claude Code's dim style and trailing spaces
    // don't get trimmed. See pipeline::wrap_for_claude_code.
    let for_ui = glassline_render::pipeline::wrap_for_claude_code(&composite);
    debug_log(debug_enabled, "stdout", &for_ui);
    // Store the wrapped-but-not-newline-terminated form in the cache; both
    // fresh-render and cache-hit branches use println!, so an explicit
    // newline lives in exactly one place (println!'s implicit \n) and the
    // two branches produce byte-identical stdout.
    render_cache::write(&cache_key, &for_ui, now_ms);
    println!("{for_ui}");
    ExitCode::SUCCESS
}

/// Pull `--config <path>` out of a raw argv slice.
fn extract_config_override(args: &[String]) -> Option<PathBuf> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--config"
            && let Some(next) = it.next()
        {
            return Some(PathBuf::from(next));
        }
    }
    None
}

fn warning_banner(reason: &str) -> StyledSpan {
    StyledSpan {
        text: format!("[glassline] {reason}"),
        fg: Color::Named("yellow".into()),
        bold: true,
        ..StyledSpan::default()
    }
}

fn run_install_cmd(args: &[String]) -> ExitCode {
    let (slug, opts) = match parse_install_args(args) {
        Ok(pair) => pair,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline install: {e}");
            print_install_help();
            return ExitCode::from(2);
        }
    };
    let Some(adapter) = ADAPTER_REGISTRY.get(slug.as_str()) else {
        let _ = writeln!(
            std::io::stderr(),
            "glassline install: unknown CLI `{slug}`.\nKnown: {known}. See `glassline install --help`.",
            known = ADAPTER_ORDER.join(", "),
        );
        return ExitCode::from(2);
    };
    match adapter.install(&opts) {
        Ok(report) => {
            print!("{}", render_report(&report, "install"));
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline install: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_uninstall_cmd(args: &[String]) -> ExitCode {
    let (slug, opts) = match parse_install_args(args) {
        Ok(pair) => pair,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline uninstall: {e}");
            print_install_help();
            return ExitCode::from(2);
        }
    };
    let Some(adapter) = ADAPTER_REGISTRY.get(slug.as_str()) else {
        let _ = writeln!(
            std::io::stderr(),
            "glassline uninstall: unknown CLI `{slug}`.\nKnown: {known}. See `glassline install --help`.",
            known = ADAPTER_ORDER.join(", "),
        );
        return ExitCode::from(2);
    };
    match adapter.uninstall(&opts) {
        Ok(report) => {
            print!("{}", render_report(&report, "uninstall"));
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline uninstall: {e}");
            ExitCode::from(1)
        }
    }
}

fn run_import_cmd(args: &[String]) -> ExitCode {
    let opts = match parse_import_args(args) {
        Ok(o) => o,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline import: {e}");
            print_import_help();
            return ExitCode::from(2);
        }
    };
    match import::run_import(&opts) {
        Ok(report) => {
            if !opts.quiet {
                print!("{}", import::render_report(&report, &opts));
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline import: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}

fn parse_import_args(args: &[String]) -> Result<ImportOpts, String> {
    let mut opts = ImportOpts::default();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--from" => {
                opts.from = Some(PathBuf::from(
                    it.next().ok_or_else(|| "--from needs a path".to_string())?,
                ));
            }
            "--to" => {
                opts.to = Some(PathBuf::from(
                    it.next().ok_or_else(|| "--to needs a path".to_string())?,
                ));
            }
            "--dry-run" => opts.dry_run = true,
            "--force" | "-f" => opts.force = true,
            "--yes" | "-y" => opts.yes = true,
            "--quiet" | "-q" => opts.quiet = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(opts)
}

fn print_import_help() {
    let _ = writeln!(
        std::io::stderr(),
        "usage: glassline import [--from <path>] [--to <path>] [--dry-run] [--force] [--yes] [--quiet]"
    );
}

/// Parse `install` / `uninstall` argv into an adapter slug + `InstallOpts`.
///
/// The slug (from `--for <slug>`) defaults to `"claude"` for backcompat
/// so `glassline install` (no flag) behaves identically to
/// `glassline install --for claude`. Unknown slugs are validated at
/// dispatch time (against `adapter::REGISTRY`), not here — this parser
/// only enforces argv shape.
fn parse_install_args(args: &[String]) -> Result<(String, InstallOpts), String> {
    let mut opts = InstallOpts::default();
    let mut slug = String::from("claude");
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--project" => opts.scope = Scope::Project,
            "--user" => opts.scope = Scope::User,
            "--absolute-path" => opts.absolute_path = true,
            "--use-path" => opts.absolute_path = false,
            "--dry-run" => opts.dry_run = true,
            "--force" | "-f" => opts.force = true,
            "--for" => {
                slug = it
                    .next()
                    .ok_or_else(|| "--for requires a CLI slug".to_string())?
                    .clone();
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok((slug, opts))
}

fn print_help() {
    println!(
        "\
glassline {version} — Claude Code status line

USAGE:
  <StatusJSON on stdin> | glassline [--config <path>]  Render a status line.
  glassline                                            Bare TTY invocation opens the editor
                                                       (`glassline-tui`) if it's installed.
  glassline install [--for <slug>] [OPTS]              Wire into a coding CLI (default: claude).
  glassline uninstall [--for <slug>] [OPTS]            Remove the wiring.
  glassline import [OPTS]                              Migrate from ccstatusline.
  glassline demo <MODE> [OPTS]                         Preview animations live.
  glassline --version                                  Print version.
  glassline --help                                     This help.

RENDER OPTS:
  --config <path>   Override the settings.json path.
                    Default order: $GLASSLINE_CONFIG > platform default
                    (~/.config/glassline/settings.json on Linux/Mac,
                     %APPDATA%\\glassline\\settings.json on Windows).

INSTALL OPTS:
  --user            Target ~/.claude/settings.json (default).
  --project         Target ./.claude/settings.json.
  --absolute-path   Write the absolute path of the exe instead of the bare
                    `glassline` name (default: bare, resolved via $PATH).
  --dry-run         Preview only; do not write.
  --force           Overwrite an existing statusLine even if it isn't glassline's.

IMPORT OPTS:
  --from <path>     Explicit ccstatusline settings.json. Skips auto-detect.
  --to <path>       Explicit glassline target. Default: platform config path.
  --dry-run         Print report + would-be JSON; do not write.
  --force           Overwrite an existing glassline settings.json.
  --yes             Skip the confirmation prompt.
  --quiet           Suppress the report; only warnings + errors on stderr.
",
        version = env!("CARGO_PKG_VERSION"),
    );
}

fn print_install_help() {
    let known = ADAPTER_ORDER.join(" | ");
    let _ = writeln!(
        std::io::stderr(),
        "usage: glassline install|uninstall [--for <{known}>] [--user|--project] [--absolute-path] [--dry-run] [--force]\n  --for <slug>   Target CLI to wire glassline into (default: claude). Known: {known}."
    );
}

/// One-line placeholder shown the very first time Claude Code invokes
/// glassline before any config exists. Tells the user how to configure —
/// which is to run `glassline` in a terminal (TTY shim → glassline-tui
/// wizard), NOT `glassline install` (that only wires the statusLine
/// hook; it doesn't create a config file).
fn first_run_slice_settings() -> Settings {
    let mut spec = WidgetSpec::new("1", "custom-text");
    spec.custom_text = Some(format!(
        "[glassline v{}] no config yet — run `glassline` in a terminal to configure",
        env!("CARGO_PKG_VERSION")
    ));
    spec.color = Some("cyan".into());
    Settings {
        lines: vec![vec![spec]],
        ..Settings::in_memory_defaults()
    }
}

fn debug_log(enabled: bool, tag: &str, msg: &str) {
    if !enabled {
        return;
    }
    let Some(path) = debug_log_path() else { return };
    let _ = std::fs::create_dir_all(path.parent().unwrap_or_else(|| std::path::Path::new(".")));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let _ = writeln!(f, "[{ts}] {tag}: {msg}");
    }
}

fn debug_log_path() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(
        PathBuf::from(home)
            .join(".cache")
            .join("glassline")
            .join("debug.log"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_install_args_defaults_to_claude() {
        let (slug, opts) = parse_install_args(&args(&[])).unwrap();
        assert_eq!(slug, "claude");
        assert_eq!(opts.scope, Scope::User);
    }

    #[test]
    fn parse_install_args_captures_for_slug() {
        let (slug, _) = parse_install_args(&args(&["--for", "codex"])).unwrap();
        assert_eq!(slug, "codex");
    }

    #[test]
    fn parse_install_args_for_without_value_errors() {
        assert!(parse_install_args(&args(&["--for"])).is_err());
    }

    #[test]
    fn parse_install_args_for_and_scope_compose() {
        let (slug, opts) = parse_install_args(&args(&["--for", "codex", "--project"])).unwrap();
        assert_eq!(slug, "codex");
        assert_eq!(opts.scope, Scope::Project);
    }

    #[test]
    fn parse_install_args_for_dry_run_compose() {
        let (slug, opts) = parse_install_args(&args(&["--for", "claude", "--dry-run"])).unwrap();
        assert_eq!(slug, "claude");
        assert!(opts.dry_run);
    }

    #[test]
    fn parse_install_args_unknown_flag_errors() {
        assert!(parse_install_args(&args(&["--nope"])).is_err());
    }
}
