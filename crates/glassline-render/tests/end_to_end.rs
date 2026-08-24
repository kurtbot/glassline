//! End-to-end vertical slice: pipe a StatusJSON payload through the binary
//! and assert the rendered output.
//!
//! Uses [`assert_cmd`] to invoke the release-mode binary via cargo's target
//! dir + [`glassline_testkit::normalise`] so we compare on visible-attribute
//! form (design §4.10) rather than raw byte sequences.

use assert_cmd::Command;
use glassline_testkit::normalise;

#[test]
fn vertical_slice_first_run_hint_points_to_glassline_terminal() {
    // No config on disk → first-run hint is shown. The hint tells the
    // user to run `glassline` in a terminal (which routes through the
    // TTY shim to the wizard). NOT `glassline install` — that command
    // only wires the statusLine hook; it doesn't create a config.
    //
    // We assert on the raw stdout rather than `normalise().visible()`
    // because the normaliser converts spaces to non-breaking-spaces
    // for visible-attribute comparison, which trips up substring
    // matches against literal ASCII strings.
    let payload = r#"{"session_id":"abc-123"}"#;
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin(payload)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    // Custom-text widget converts spaces to U+00A0 (non-breaking) at
    // render time so terminals don't break mid-widget. Reverse that
    // for our assertions.
    let normalised = stdout.replace('\u{a0}', " ");
    assert!(
        normalised.contains("no config yet"),
        "expected 'no config yet' hint in {normalised:?}"
    );
    assert!(
        normalised.contains("run `glassline` in a terminal"),
        "expected 'run `glassline` in a terminal' guidance in {normalised:?}",
    );
    assert!(
        !normalised.contains("`glassline install`"),
        "hint must not point at `glassline install` (that only wires the hook, doesn't create config): {normalised:?}",
    );
}

#[test]
fn version_flag_shorts_out() {
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .arg("--version")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.starts_with("glassline "));
}

#[test]
fn empty_stdin_prints_marker_and_succeeds() {
    // Design choice: never emit an empty status line — Claude Code will just
    // show blank space, which looks broken. Always print at least a version
    // marker so the user sees glassline is alive.
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin("")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("no stdin"),
        "expected marker, got {stdout:?}"
    );
}

#[test]
fn malformed_json_prints_marker_and_succeeds() {
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin("{not-json")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    // Marker text changed after the P3b adapter-dispatch refactor:
    // pre-refactor emitted `[glassline v... parse-err]`, post-refactor
    // emits `[glassline v... claude-err] parse StatusJSON from stdin: ...`
    // (per-adapter slug so Codex/Grok errors are distinguishable). The
    // durable contract is "some -err] marker + exit 0", not the specific
    // string.
    assert!(
        stdout.contains("err]") && stdout.contains("parse"),
        "expected parse-related err marker, got {stdout:?}",
    );
}

#[test]
fn install_unknown_for_slug_exits_2_with_known_slugs_hint() {
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .args(["install", "--for", "bogus-cli", "--dry-run"])
        .assert()
        .failure();
    let code = assert.get_output().status.code();
    assert_eq!(code, Some(2), "expected exit 2 for unknown slug");
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("unknown CLI"),
        "expected 'unknown CLI' in stderr, got: {stderr:?}"
    );
    assert!(
        stderr.contains("claude"),
        "expected 'claude' listed as known slug in stderr, got: {stderr:?}"
    );
}

#[test]
fn codex_home_env_routes_to_codex_adapter() {
    // With CODEX_HOME set, env_var_dispatch routes stdin through
    // CodexAdapter. Malformed JSON hits its `parse_forward_compat_statusline`
    // path and errors; the `codex-err]` marker proves the dispatch
    // routed (would have been `claude-err]` if it fell back).
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env(
            "CODEX_HOME",
            std::env::temp_dir().join("glassline-e2e-codex-dispatch"),
        )
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin("{malformed")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("codex-err]"),
        "expected codex-err marker (adapter routed), got: {stdout:?}"
    );
}

#[test]
fn grok_home_env_routes_to_grok_adapter() {
    // With GROK_HOME set, env_var_dispatch routes stdin through
    // GrokAdapter. Empty temp dir = no `signals.json`, so read_context
    // errors with "no signals.json at ...". The `grok-err]` marker
    // proves the dispatch routed.
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env(
            "GROK_HOME",
            std::env::temp_dir().join("glassline-e2e-grok-dispatch"),
        )
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin("{malformed")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("grok-err]"),
        "expected grok-err marker (adapter routed), got: {stdout:?}"
    );
    assert!(
        stdout.contains("no signals.json"),
        "expected 'no signals.json' error, got: {stdout:?}"
    );
}

#[test]
fn install_print_caveats_prints_unsupported_widgets_for_codex() {
    // The wizard's batched install path shells out `install --for
    // <slug> --print-caveats` after each successful install to
    // decorate its summary modal. This test asserts the output shape
    // — one widget kind per line, exit 0.
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .args(["install", "--for", "codex", "--print-caveats"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let widgets: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert!(
        widgets.contains(&"block-timer"),
        "codex caveats include block-timer, got: {stdout:?}"
    );
    assert!(
        widgets.contains(&"session-usage"),
        "codex caveats include session-usage, got: {stdout:?}"
    );
}

#[test]
fn install_print_caveats_returns_empty_for_claude() {
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .args(["install", "--for", "claude", "--print-caveats"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    // Claude drives the full canonical catalog — no unsupported widgets.
    assert!(
        stdout.trim().is_empty(),
        "claude has no caveats; expected empty stdout, got: {stdout:?}"
    );
}

#[test]
fn install_for_codex_dry_run_reports_plugin_json_path() {
    // Run against an isolated CODEX_HOME so we don't touch the
    // developer's actual ~/.codex. --dry-run guarantees no writes
    // even if the isolation failed.
    let temp = std::env::temp_dir().join(format!(
        "glassline-e2e-codex-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ));
    let _ = std::fs::remove_dir_all(&temp);

    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .args(["install", "--for", "codex", "--dry-run"])
        .env("CODEX_HOME", &temp)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(
        stdout.contains("dry-run"),
        "expected dry-run marker in stdout, got: {stdout:?}"
    );
    assert!(
        stdout.contains("plugin.json") || stdout.contains("plugins"),
        "expected plugin path or 'plugins' dir in dry-run output, got: {stdout:?}"
    );
    // Clean up in case anything did leak.
    let _ = std::fs::remove_dir_all(&temp);
}

#[test]
fn install_for_claude_dry_run_matches_bare_install() {
    // Backcompat invariant: `install --for claude` must behave
    // identically to `install` (no --for). Same exit code, same
    // dispatch through the ClaudeAdapter → run_install path.
    let bare = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .args(["install", "--dry-run"])
        .assert()
        .success();
    let bare_stdout = String::from_utf8_lossy(&bare.get_output().stdout).into_owned();

    let explicit = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .args(["install", "--for", "claude", "--dry-run"])
        .assert()
        .success();
    let explicit_stdout = String::from_utf8_lossy(&explicit.get_output().stdout).into_owned();

    assert_eq!(
        bare_stdout, explicit_stdout,
        "install --for claude must be byte-identical to install (no --for)",
    );
}

#[test]
fn missing_session_id_still_renders_version_prefix() {
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin(r#"{"cwd":"/tmp"}"#)
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let normalised = normalise(&stdout);
    assert!(normalised.visible().contains("glassline"));
}
