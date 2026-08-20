//! `glassline demo` — in-place animation preview so users can watch
//! their settings render across a range of synthetic inputs without
//! having to wait for Claude Code's next refresh.
//!
//! Loads the user's real `settings.json` (same resolution as
//! [`crate::config::load`]) and renders a synthesized [`StatusJson`]
//! whose relevant fields sweep over time. The frame is redrawn in
//! place via cursor-up + erase-to-end so terminals show smooth
//! motion instead of a scrolling log.
//!
//! Modes:
//! - `threshold`  — used_percentage sweeps 0 → 100 → 0 (default 20s).
//! - `rainbow`    — fills in a fixed 42% context and lets `now_ms` drive
//!   any `animate:"rainbow"` widget you have set up.
//! - `sweep`      — same but exercises the sweep-gradient path.
//! - `pulse`      — pulse-brightness at 42%.
//! - `all`        — plays each mode for N seconds in sequence.
//!
//! Flags (all optional):
//! `--seconds N`   total duration per mode (default 20).
//! `--fps N`       frames per second (default 6). Clamped 1..=60.
//! `--config PATH` override settings.json path (same as render mode).

use std::{
    io::Write,
    path::PathBuf,
    process::ExitCode,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use glassline_core::{
    render_context::RenderContext,
    settings::Settings,
    status_json::{ContextWindow, Cost, Effort, ModelInfo, StatusJson},
    widget::WidgetRequirements,
};

use crate::{
    config::{LoadOutcome, load},
    pipeline::{compute_requirements, render_to_string, wrap_for_claude_code},
    transcript, usage,
};

/// Entry point invoked from `main.rs` when argv starts with `demo`.
pub fn run(args: &[String]) -> ExitCode {
    let mode = args.first().map(String::as_str).unwrap_or("threshold");
    let seconds = flag_u64(args, "--seconds").unwrap_or(20);
    let fps = flag_u64(args, "--fps").unwrap_or(6).clamp(1, 60);
    let config_override = flag_str(args, "--config").map(PathBuf::from);

    match mode {
        "threshold" | "context" | "pct" => run_threshold(seconds, fps, config_override.as_deref()),
        "rainbow" => run_static(seconds, fps, 42.0, config_override.as_deref()),
        "sweep" => run_static(seconds, fps, 42.0, config_override.as_deref()),
        "pulse" => run_static(seconds, fps, 42.0, config_override.as_deref()),
        "all" => run_all(seconds, fps, config_override.as_deref()),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("glassline demo: unknown mode `{other}`");
            print_help();
            ExitCode::from(2)
        }
    }
}

fn run_threshold(seconds: u64, fps: u64, config_override: Option<&std::path::Path>) -> ExitCode {
    let Some(settings) = load_settings(config_override) else {
        return ExitCode::from(1);
    };
    let total_frames = seconds * fps;
    let frame_ms = 1000 / fps;
    let mut prev_lines = 0usize;

    println!("glassline demo threshold — sweeping used_percentage 0 → 100 → 0 over {seconds}s");
    println!();
    let mut stdout = std::io::stdout().lock();

    for i in 0..total_frames {
        // Triangle wave 0 -> 1 -> 0 across the total range so each band
        // gets visited twice; the flashing top band shows up on both
        // legs.
        let t = i as f64 / total_frames.max(1) as f64;
        let pct = if t < 0.5 { t * 2.0 } else { (1.0 - t) * 2.0 } * 100.0;
        let frame = render_frame(&settings, pct);
        if prev_lines > 0 {
            let _ = write!(stdout, "\x1b[{prev_lines}A\x1b[J");
        }
        let annotation = format!("used_percentage = {pct:5.1}%");
        let _ = writeln!(stdout, "{annotation}");
        let _ = writeln!(stdout, "{frame}");
        prev_lines = frame.lines().count() + 1;
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(frame_ms));
    }
    println!();
    println!("done.");
    ExitCode::SUCCESS
}

fn run_static(
    seconds: u64,
    fps: u64,
    fixed_pct: f64,
    config_override: Option<&std::path::Path>,
) -> ExitCode {
    let Some(settings) = load_settings(config_override) else {
        return ExitCode::from(1);
    };
    let total_frames = seconds * fps;
    let frame_ms = 1000 / fps;
    let mut prev_lines = 0usize;

    println!(
        "glassline demo — fixed used_percentage={fixed_pct}%, watching animate:* effects over {seconds}s",
    );
    println!();
    let mut stdout = std::io::stdout().lock();

    for _ in 0..total_frames {
        let frame = render_frame(&settings, fixed_pct);
        if prev_lines > 0 {
            let _ = write!(stdout, "\x1b[{prev_lines}A\x1b[J");
        }
        let _ = writeln!(stdout, "{frame}");
        prev_lines = frame.lines().count();
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(frame_ms));
    }
    println!();
    println!("done.");
    ExitCode::SUCCESS
}

fn run_all(seconds: u64, fps: u64, config_override: Option<&std::path::Path>) -> ExitCode {
    for mode in ["threshold", "rainbow", "sweep", "pulse"] {
        eprintln!("\n== {mode} ==");
        let code = match mode {
            "threshold" => run_threshold(seconds, fps, config_override),
            _ => run_static(seconds, fps, 42.0, config_override),
        };
        if code != ExitCode::SUCCESS {
            return code;
        }
    }
    ExitCode::SUCCESS
}

fn load_settings(override_path: Option<&std::path::Path>) -> Option<Settings> {
    match load(override_path) {
        Ok(loaded) => match loaded.outcome {
            LoadOutcome::Loaded => Some(loaded.settings),
            LoadOutcome::FirstRun => {
                eprintln!(
                    "glassline demo: no settings.json at {}. Falling back to defaults.",
                    loaded.path.display()
                );
                Some(loaded.settings)
            }
            LoadOutcome::CorruptFallback { reason } => {
                eprintln!("glassline demo: settings.json unreadable: {reason}");
                Some(loaded.settings)
            }
        },
        Err(e) => {
            eprintln!("glassline demo: cannot resolve config path: {e}");
            None
        }
    }
}

/// Synthesize a StatusJson at `pct` used-context and render one frame
/// through the full pipeline. Returns the Claude-Code-wrapped string
/// ready to write to stdout.
fn render_frame(settings: &Settings, pct: f64) -> String {
    let requirements = compute_requirements(settings);
    let payload = synth_payload(pct);
    let now_ms = current_time_ms();

    let mut ctx = RenderContext {
        data: Some(payload.clone()),
        now_ms,
        ..RenderContext::default()
    };

    // Only touch the network on USAGE — otherwise the demo blocks 5s
    // waiting for HTTPS timeouts every frame. Skip usage; widgets will
    // render '[No credentials]' or empty which is fine for a demo.
    let want_speed = requirements.contains(WidgetRequirements::SPEED);
    let want_compaction = requirements.contains(WidgetRequirements::COMPACTION);
    let want_transcript = requirements.contains(WidgetRequirements::TRANSCRIPT);
    let want_session_clock = requirements.contains(WidgetRequirements::SESSION_CLOCK);
    // Synthesize a small in-memory transcript if any speed / compaction /
    // token widget wants one.
    if (want_speed || want_compaction || want_transcript || want_session_clock)
        && let Some(path) = ensure_demo_transcript()
        && let Ok(scan) = transcript::scan(
            &path,
            requirements
                & (WidgetRequirements::TRANSCRIPT
                    | WidgetRequirements::COMPACTION
                    | WidgetRequirements::SESSION_CLOCK
                    | WidgetRequirements::SPEED),
        )
    {
        ctx.token_metrics = Some(scan.tokens);
        ctx.compaction_data = Some(scan.compaction);
        ctx.session_duration = scan.session_duration;
        ctx.speed_metrics = Some(scan.speed);
    }
    if requirements.contains(WidgetRequirements::USAGE)
        && let Some(u) = usage::fetch_or_cached()
    {
        ctx.usage_data = Some(u);
    }

    let out = render_to_string(ctx, settings).unwrap_or_default();
    wrap_for_claude_code(&out)
}

/// A hardcoded 6-turn transcript so speed / compaction widgets have
/// something to chew on during the demo. Written once per process to
/// a per-PID tempfile.
fn ensure_demo_transcript() -> Option<PathBuf> {
    let dir = std::env::temp_dir().join(format!("glassline-demo-{}", std::process::id()));
    let path = dir.join("transcript.jsonl");
    if path.exists() {
        return Some(path);
    }
    std::fs::create_dir_all(&dir).ok()?;
    let synthetic = r#"{"type":"user","timestamp":"2026-08-18T10:00:00Z"}
{"type":"assistant","timestamp":"2026-08-18T10:00:05Z","message":{"usage":{"input_tokens":42000,"output_tokens":1500,"cache_read_input_tokens":18000},"stop_reason":"end_turn"}}
{"type":"user","timestamp":"2026-08-18T10:05:00Z"}
{"type":"assistant","timestamp":"2026-08-18T10:05:08Z","message":{"usage":{"input_tokens":30000,"output_tokens":2000,"cache_read_input_tokens":8000},"stop_reason":"end_turn"}}
{"type":"user","timestamp":"2026-08-18T10:10:00Z"}
{"type":"assistant","timestamp":"2026-08-18T10:10:12Z","message":{"usage":{"input_tokens":25000,"output_tokens":1800,"cache_read_input_tokens":5000},"stop_reason":"end_turn"}}
"#;
    std::fs::write(&path, synthetic).ok()?;
    Some(path)
}

fn synth_payload(pct: f64) -> StatusJson {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|p| p.to_str().map(String::from));
    let transcript_path = ensure_demo_transcript().and_then(|p| p.to_str().map(String::from));
    StatusJson {
        session_id: Some("demo".into()),
        transcript_path,
        cwd,
        model: Some(ModelInfo::Full {
            id: Some("claude-opus-4-7".into()),
            display_name: Some("Opus 4.7".into()),
        }),
        cost: Some(Cost {
            total_cost_usd: Some(4.20),
            total_duration_ms: Some(3_720_000.0),
            ..Default::default()
        }),
        effort: Some(Effort {
            level: Some("xhigh".into()),
        }),
        context_window: Some(ContextWindow {
            context_window_size: Some(1_000_000.0),
            used_percentage: Some(pct),
            total_input_tokens: Some(pct * 10_000.0),
            total_output_tokens: Some(pct * 500.0),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn flag_u64(args: &[String], key: &str) -> Option<u64> {
    flag_str(args, key).and_then(|s| s.parse().ok())
}

fn flag_str<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == key
            && let Some(v) = it.next()
        {
            return Some(v.as_str());
        }
    }
    None
}

fn print_help() {
    println!(
        "\
glassline demo — preview animations without waiting for Claude Code refreshes

USAGE:
  glassline demo <MODE> [OPTS]

MODES:
  threshold  Sweeps context used_percentage 0 → 100 → 0 (default).
             Watch the color bands trigger on any threshold widget.
  rainbow    Holds context at 42%; drives animate:\"rainbow\" widgets.
  sweep      Holds context at 42%; drives animate:\"sweep\" widgets.
  pulse      Holds context at 42%; drives animate:\"pulse\" widgets.
  all        Plays each mode in sequence.

OPTS:
  --seconds N     Duration per mode (default 20).
  --fps N         Frames per second (1..=60, default 6).
  --config PATH   Override settings.json path.
  --help          This help.

Terminal control: each frame overwrites the previous one via cursor-up
and erase-to-end; run in a real terminal for smooth output.
"
    );
}
