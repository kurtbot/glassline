// SPDX-FileCopyrightText: 2026 Kurt Milan
// SPDX-License-Identifier: MIT

//! `glassline` — hot-path binary entry point.
//!
//! Argv handled by a hand-rolled parser (clap stays out of the hot path per
//! design §4.1). Subcommands:
//!   * `install`   / `uninstall`   — wire glassline into Claude Code
//!   * `--version` / `-V`          — print version + exit
//!   * `--help`    / `-h`          — usage
//!   * `--config <path>`           — override settings.json path
//!
//! Render mode (no subcommand): slurp stdin, load config, render pipeline,
//! write to stdout. On the P1 slice a **first-run** load falls back to a
//! small marker widget so the user sees glassline is alive even before they
//! customise anything. Once real MVP widgets ship (T-1.7+), the first-run
//! defaults will produce a useful line on their own.

use std::{io::Write, path::PathBuf, process::ExitCode};

use glassline_core::{
    color::Color,
    render_context::RenderContext,
    settings::{Settings, WidgetSpec},
    span::StyledSpan,
    status_json::StatusJson,
    widget::WidgetRequirements,
};
use glassline_render::{
    ansi::spans_to_string,
    config::{LoadOutcome, default_settings_path, load},
    import::{self, ImportOpts},
    install::{InstallOpts, Scope, render_report, run_install, run_uninstall},
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
            run_render(&raw_args)
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
        | WidgetRequirements::SPEED;
    if let Some(transcript_path) = payload.transcript_path.as_deref()
        && (requirements.contains(WidgetRequirements::TRANSCRIPT)
            || requirements.contains(WidgetRequirements::COMPACTION)
            || requirements.contains(WidgetRequirements::SESSION_CLOCK)
            || requirements.contains(WidgetRequirements::SPEED))
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
    let opts = match parse_install_args(args) {
        Ok(o) => o,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline install: {e}");
            print_install_help();
            return ExitCode::from(2);
        }
    };
    match run_install(&opts) {
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
    let opts = match parse_install_args(args) {
        Ok(o) => o,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "glassline uninstall: {e}");
            print_install_help();
            return ExitCode::from(2);
        }
    };
    match run_uninstall(&opts) {
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

fn parse_install_args(args: &[String]) -> Result<InstallOpts, String> {
    let mut opts = InstallOpts::default();
    for arg in args {
        match arg.as_str() {
            "--project" => opts.scope = Scope::Project,
            "--user" => opts.scope = Scope::User,
            "--absolute-path" => opts.absolute_path = true,
            "--use-path" => opts.absolute_path = false,
            "--dry-run" => opts.dry_run = true,
            "--force" | "-f" => opts.force = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(opts)
}

fn print_help() {
    println!(
        "\
glassline {version} — Claude Code status line

USAGE:
  <StatusJSON on stdin> | glassline [--config <path>]  Render a status line.
  glassline install [OPTS]                             Wire into Claude Code.
  glassline uninstall [OPTS]                           Remove the wiring.
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
    let _ = writeln!(
        std::io::stderr(),
        "usage: glassline install|uninstall [--user|--project] [--absolute-path] [--dry-run] [--force]"
    );
}

/// A single-widget "hello, glassline" placeholder for the very first launch
/// (no settings.json on disk yet). Once T-1.7 lands real MVP widgets this
/// branch dies; users will see the TS-parity default line instead.
fn first_run_slice_settings() -> Settings {
    let mut spec = WidgetSpec::new("1", "custom-text");
    spec.custom_text = Some(format!(
        "[glassline v{}] {{session_id}} — no config, run `glassline install`",
        env!("CARGO_PKG_VERSION")
    ));
    spec.color = Some("cyan".into());
    Settings {
        lines: vec![vec![spec], vec![], vec![]],
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
