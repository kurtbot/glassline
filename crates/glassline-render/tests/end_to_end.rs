//! End-to-end vertical slice: pipe a StatusJSON payload through the binary
//! and assert the rendered output.
//!
//! Uses [`assert_cmd`] to invoke the release-mode binary via cargo's target
//! dir + [`glassline_testkit::normalise`] so we compare on visible-attribute
//! form (design §4.10) rather than raw byte sequences.

use assert_cmd::Command;
use glassline_testkit::normalise;

#[test]
fn vertical_slice_renders_session_id_from_stdin() {
    let payload = r#"{"session_id":"abc-123"}"#;
    let assert = Command::cargo_bin("glassline")
        .expect("built glassline binary")
        .env("GLASSLINE_CONFIG", "/no/such/glassline/config.json")
        .write_stdin(payload)
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let normalised = normalise(&stdout);
    let visible = normalised.visible();
    assert!(
        visible.contains("abc-123"),
        "expected session id in {visible:?}"
    );
    assert!(
        visible.contains("glassline"),
        "expected version marker in {visible:?}",
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
    assert!(
        stdout.contains("parse-err"),
        "expected parse-err marker, got {stdout:?}",
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
