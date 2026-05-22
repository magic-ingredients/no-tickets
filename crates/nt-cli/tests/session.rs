//! Integration tests for `no-tickets session start | show | end`.
//!
//! Exercises the binary end-to-end via `assert_cmd`. Unit-level coverage
//! of the session/state modules lives alongside them under
//! `src/{session,state}.rs::tests`. These tests pin the CLI surface:
//! exit codes, JSON output shape, and the filesystem side-effects an
//! agent harness can rely on.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

fn nt() -> Command {
    Command::cargo_bin("no-tickets").expect("binary built")
}

fn isolate<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("NO_TICKETS_HOME", home)
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
        .env_remove("NO_TICKETS_SESSION_FILE")
}

fn session_path(home: &Path) -> std::path::PathBuf {
    home.join(".notickets").join("active-session.json")
}

fn state_path(home: &Path) -> std::path::PathBuf {
    home.join(".notickets").join("state.json")
}

fn parse_stdout_json(output: &std::process::Output) -> Value {
    let stdout = String::from_utf8(output.stdout.clone()).expect("utf8 stdout");
    serde_json::from_str(stdout.trim()).expect("valid JSON")
}

fn contains_null_value(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Object(m) => m.values().any(contains_null_value),
        Value::Array(a) => a.iter().any(contains_null_value),
        _ => false,
    }
}

// ─── session start ──────────────────────────────────────────────────────────

#[test]
fn session_start_with_only_agent_writes_minimal_actor_file() {
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args(["session", "start", "--agent", "claude"])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "session start should succeed; stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );

    let path = session_path(temp.path());
    assert!(path.exists(), "active-session.json must be written");
    let raw = fs::read_to_string(&path).unwrap();
    let parsed: Value = serde_json::from_str(&raw).expect("valid JSON");

    assert_eq!(parsed["version"], 1);
    assert_eq!(parsed["actor"]["type"], "agent");
    assert_eq!(parsed["actor"]["agentId"], "claude");
    assert!(
        parsed["actor"].get("model").is_none(),
        "model omitted when not supplied; got {raw}",
    );
    assert!(parsed["actor"].get("provider").is_none());
    assert!(parsed["actor"].get("thinkingEffort").is_none());
    assert!(parsed["actor"].get("sessionId").is_none());
    assert!(parsed.get("startedAt").is_some(), "startedAt stamped");
    assert!(parsed.get("pid").is_some(), "pid recorded");
    assert_eq!(parsed["maxAgeHours"], 24, "default max-age-hours = 24");

    // No `null` literals or "n/a" sentinels anywhere on disk.
    assert!(!raw.contains("null"), "no null in file; got {raw}");
    assert!(!raw.contains("\"n/a\""), "no n/a sentinel; got {raw}");
}

#[test]
fn session_start_with_full_actor_writes_all_fields() {
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--model",
            "claude-opus-4-7",
            "--provider",
            "anthropic",
            "--thinking-effort",
            "high",
            "--session-id",
            "sess-abc123",
            "--max-age-hours",
            "48",
        ])
        .output()
        .expect("spawn");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(session_path(temp.path())).unwrap()).unwrap();
    assert_eq!(parsed["actor"]["agentId"], "claude");
    assert_eq!(parsed["actor"]["model"], "claude-opus-4-7");
    assert_eq!(parsed["actor"]["provider"], "anthropic");
    assert_eq!(parsed["actor"]["thinkingEffort"], "high");
    assert_eq!(parsed["actor"]["sessionId"], "sess-abc123");
    assert_eq!(parsed["maxAgeHours"], 48);
}

#[test]
fn session_start_requires_agent_flag() {
    // clap should reject invocations missing `--agent`.
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args(["session", "start"])
        .output()
        .expect("spawn");
    assert!(
        !output.status.success(),
        "missing --agent must fail; stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn session_start_rejects_max_age_hours_above_seven_days() {
    // PRD: hard cap 168h (7 days). clap must reject anything above.
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--max-age-hours",
            "169",
        ])
        .output()
        .expect("spawn");
    assert!(
        !output.status.success(),
        "--max-age-hours 169 must be rejected; stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn session_start_accepts_max_age_hours_at_seven_day_boundary() {
    // 168h = 7 days exactly — the documented cap, still valid.
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--max-age-hours",
            "168",
        ])
        .assert()
        .success();
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(session_path(temp.path())).unwrap()).unwrap();
    assert_eq!(parsed["maxAgeHours"], 168);
}

#[test]
fn session_start_rejects_zero_max_age_hours() {
    // A zero-hour session would be insta-expired and is useless;
    // clap's `range(1..=168)` must reject it at parse time.
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--max-age-hours",
            "0",
        ])
        .output()
        .expect("spawn");
    assert!(
        !output.status.success(),
        "--max-age-hours 0 must be rejected"
    );
}

#[test]
fn session_start_rejects_invalid_thinking_effort() {
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--thinking-effort",
            "very-high",
        ])
        .output()
        .expect("spawn");
    assert!(
        !output.status.success(),
        "invalid --thinking-effort must fail",
    );
}

#[test]
fn session_start_overwrites_existing_session() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args(["session", "start", "--agent", "claude"])
        .assert()
        .success();
    isolate(&mut nt(), temp.path())
        .args(["session", "start", "--agent", "codex"])
        .assert()
        .success();
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(session_path(temp.path())).unwrap()).unwrap();
    assert_eq!(
        parsed["actor"]["agentId"], "codex",
        "second start overwrites first",
    );
}

// ─── session show ───────────────────────────────────────────────────────────

#[test]
fn session_show_reports_inactive_when_no_session_file() {
    let temp = tempfile::tempdir().unwrap();
    let output = isolate(&mut nt(), temp.path())
        .args(["session", "show"])
        .output()
        .expect("spawn");
    assert!(output.status.success());
    let v = parse_stdout_json(&output);
    assert_eq!(v["active"], Value::Bool(false));
    assert!(
        v.get("actor").is_none(),
        "no actor key when inactive; got {v}",
    );
}

#[test]
fn session_show_reports_active_after_start() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--model",
            "claude-opus-4-7",
        ])
        .assert()
        .success();
    let output = isolate(&mut nt(), temp.path())
        .args(["session", "show"])
        .output()
        .expect("spawn");
    assert!(output.status.success());
    let v = parse_stdout_json(&output);
    assert_eq!(v["active"], Value::Bool(true));
    assert_eq!(v["actor"]["agentId"], "claude");
    assert_eq!(v["actor"]["model"], "claude-opus-4-7");
    assert!(v.get("startedAt").is_some());
    assert_eq!(
        v["expired"],
        Value::Bool(false),
        "freshly-started session is not expired",
    );
    // The serialised `session show` output must mirror the on-disk
    // "no nulls" pin — optional actor fields that weren't supplied
    // (provider, thinkingEffort, sessionId) stay absent rather than
    // showing up as JSON null.
    assert!(
        !contains_null_value(&v),
        "session show JSON must not contain null values; got {v}",
    );
}

#[test]
fn session_show_startedat_uses_millisecond_z_wire_format() {
    // The startedAt stamp written by `session start` must match the
    // schema doc-comment shape: YYYY-MM-DDTHH:mm:ss.sssZ (millisecond
    // precision, literal `Z`). Pinned by length + per-position checks
    // so consumers can rely on the format without a tolerance window.
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args(["session", "start", "--agent", "claude"])
        .assert()
        .success();
    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(session_path(temp.path())).unwrap()).unwrap();
    let started_at = parsed["startedAt"].as_str().expect("startedAt is string");
    assert_matches_millisecond_z(started_at);
}

/// Asserts `s` is shaped exactly `YYYY-MM-DDTHH:mm:ss.sssZ` (24 ASCII
/// bytes; digits in every numeric slot; `-`/`T`/`:`/`.`/`Z` separators
/// in fixed positions).
fn assert_matches_millisecond_z(s: &str) {
    assert_eq!(s.len(), 24, "wrong length for ISO-ms-Z; got {s:?}");
    let b = s.as_bytes();
    let digit_at = |i: usize| b[i].is_ascii_digit();
    for i in [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 22] {
        assert!(digit_at(i), "expected digit at {i} in {s:?}");
    }
    assert_eq!(b[4], b'-', "expected `-` at 4 in {s:?}");
    assert_eq!(b[7], b'-', "expected `-` at 7 in {s:?}");
    assert_eq!(b[10], b'T', "expected `T` at 10 in {s:?}");
    assert_eq!(b[13], b':', "expected `:` at 13 in {s:?}");
    assert_eq!(b[16], b':', "expected `:` at 16 in {s:?}");
    assert_eq!(b[19], b'.', "expected `.` at 19 in {s:?}");
    assert_eq!(b[23], b'Z', "expected `Z` at 23 in {s:?}");
}

#[test]
fn session_show_flags_expired_when_startedat_is_stale() {
    // Manually seed an active-session.json with a past startedAt.
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("active-session.json"),
        r#"{
            "version": 1,
            "actor": {"type":"agent","agentId":"claude"},
            "startedAt": "2000-01-01T00:00:00.000Z",
            "pid": 1,
            "maxAgeHours": 24
        }"#,
    )
    .unwrap();

    let output = isolate(&mut nt(), temp.path())
        .args(["session", "show"])
        .output()
        .expect("spawn");
    assert!(output.status.success());
    let v = parse_stdout_json(&output);
    assert_eq!(v["active"], Value::Bool(true));
    assert_eq!(
        v["expired"],
        Value::Bool(true),
        "stale startedAt must report expired=true",
    );
}

// ─── session end ────────────────────────────────────────────────────────────

#[test]
fn session_end_deletes_the_session_file() {
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args(["session", "start", "--agent", "claude"])
        .assert()
        .success();
    assert!(session_path(temp.path()).exists());

    isolate(&mut nt(), temp.path())
        .args(["session", "end"])
        .assert()
        .success();
    assert!(
        !session_path(temp.path()).exists(),
        "active-session.json deleted",
    );
}

#[test]
fn session_end_is_idempotent_when_nothing_to_clean_up() {
    // No session, no state.json. `end` must still succeed (exit 0).
    let temp = tempfile::tempdir().unwrap();
    isolate(&mut nt(), temp.path())
        .args(["session", "end"])
        .assert()
        .success();
    // Must NOT have created active-session.json — `end` on an absent
    // session is a no-op, not a create-then-delete dance.
    assert!(
        !session_path(temp.path()).exists(),
        "active-session.json must not be created by `end`",
    );
    // Must NOT have created state.json just to write a false flag.
    assert!(
        !state_path(temp.path()).exists(),
        "state.json must not be created by `end`; \
         the file is only written when the hint actually fires",
    );
}

#[test]
fn session_end_clears_first_publish_hint_marker() {
    // Seed state.json with the flag set. `session end` must clear it.
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("state.json"), r#"{"firstPublishHintShown":true}"#).unwrap();

    isolate(&mut nt(), temp.path())
        .args(["session", "end"])
        .assert()
        .success();

    // state.json may either be rewritten with the flag cleared, or its
    // absence may be acceptable — but if it exists, the flag must be
    // gone. Read and check.
    let path = state_path(temp.path());
    assert!(
        path.exists(),
        "state.json must remain so other state survives"
    );
    let parsed: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let flag = parsed
        .get("firstPublishHintShown")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(!flag, "firstPublishHintShown must be cleared after `end`");
}

#[test]
fn session_end_preserves_other_keys_in_state_json() {
    // Other CLI state must survive an `end` call.
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join(".notickets");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("state.json"),
        r#"{"firstPublishHintShown":true,"experimental":{"keep":"me"}}"#,
    )
    .unwrap();

    isolate(&mut nt(), temp.path())
        .args(["session", "end"])
        .assert()
        .success();

    let parsed: Value =
        serde_json::from_str(&fs::read_to_string(state_path(temp.path())).unwrap()).unwrap();
    assert_eq!(parsed["experimental"]["keep"], "me");
}

// ─── round-trip ─────────────────────────────────────────────────────────────

#[test]
fn round_trip_start_show_end() {
    let temp = tempfile::tempdir().unwrap();

    isolate(&mut nt(), temp.path())
        .args([
            "session",
            "start",
            "--agent",
            "claude",
            "--model",
            "claude-opus-4-7",
        ])
        .assert()
        .success();

    let show = isolate(&mut nt(), temp.path())
        .args(["session", "show"])
        .output()
        .expect("spawn");
    let v = parse_stdout_json(&show);
    assert_eq!(v["actor"]["agentId"], "claude");

    isolate(&mut nt(), temp.path())
        .args(["session", "end"])
        .assert()
        .success();

    let show_after = isolate(&mut nt(), temp.path())
        .args(["session", "show"])
        .output()
        .expect("spawn");
    let v_after = parse_stdout_json(&show_after);
    assert_eq!(v_after["active"], Value::Bool(false));
}
