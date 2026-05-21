//! `active-session.json` — agent harness identity for opt-in actor
//! attribution.
//!
//! Written by `no-tickets session start`. Read by `no-tickets session show`
//! and by the publish-time actor resolver (Task 5). Atomic write via
//! temp + rename so concurrent readers never see a half-written file.
//!
//! Schema:
//! ```json
//! {
//!   "version": 1,
//!   "actor": { "type": "agent", "agentId": "claude", ... },
//!   "startedAt": "2026-05-21T10:00:00.000Z",
//!   "pid": 12345,
//!   "maxAgeHours": 24
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use time::format_description::well_known::Iso8601;
use time::{Duration, OffsetDateTime};

use crate::env::Env;
use crate::paths;

pub const SESSION_FILE: &str = "active-session.json";
pub const SESSION_VERSION: u32 = 1;

/// Agent variant of the actor block. Only `agent_id` is mandatory; every
/// other field is `Option` and omitted from the serialised form when
/// `None` (no sentinel strings like `"n/a"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentActor {
    /// Discriminator — always `"agent"` for session-start-produced actors.
    #[serde(rename = "type")]
    pub actor_type: String,
    #[serde(rename = "agentId")]
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none", default)]
    pub session_id: Option<String>,
    #[serde(
        rename = "thinkingEffort",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub thinking_effort: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFile {
    pub version: u32,
    pub actor: AgentActor,
    #[serde(rename = "startedAt")]
    pub started_at: String,
    pub pid: u32,
    #[serde(rename = "maxAgeHours")]
    pub max_age_hours: u32,
}

#[derive(Debug)]
#[allow(dead_code)] // wired into nt error envelope in Task 5
pub enum SessionError {
    HomeUnresolvable,
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::HomeUnresolvable => write!(f, "Could not resolve config directory."),
            SessionError::Io(e) => write!(f, "active-session.json I/O error: {e}"),
            SessionError::Json(e) => write!(f, "active-session.json parse error: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<std::io::Error> for SessionError {
    fn from(e: std::io::Error) -> Self {
        SessionError::Io(e)
    }
}

impl From<serde_json::Error> for SessionError {
    fn from(e: serde_json::Error) -> Self {
        SessionError::Json(e)
    }
}

pub fn read(env: &dyn Env) -> Result<Option<SessionFile>, SessionError> {
    let path = session_path(env).ok_or(SessionError::HomeUnresolvable)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn write(env: &dyn Env, sf: &SessionFile) -> Result<(), SessionError> {
    let path = session_path(env).ok_or(SessionError::HomeUnresolvable)?;
    let body = serde_json::to_string_pretty(sf)?;
    crate::atomic_write::write_atomic(&path, body.as_bytes())?;
    Ok(())
}

pub fn delete(env: &dyn Env) -> Result<(), SessionError> {
    let path = session_path(env).ok_or(SessionError::HomeUnresolvable)?;
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

/// Whether the session is past its expiry window. A malformed `started_at`
/// counts as expired (defensive — better to refuse a corrupt session than
/// to authenticate against garbage). Boundary semantics are strict-greater:
/// `now == started_at + max_age_hours` is still in-window.
pub fn is_expired(started_at: &str, max_age_hours: u32, now: OffsetDateTime) -> bool {
    let Ok(parsed) = OffsetDateTime::parse(started_at, &Iso8601::DEFAULT) else {
        return true;
    };
    now > parsed + Duration::hours(i64::from(max_age_hours))
}

/// Format an instant as `YYYY-MM-DDTHH:mm:ss.sssZ` — millisecond
/// precision, literal `Z` for UTC. Pinned for the `startedAt` wire
/// shape on `active-session.json` so the file stays canonical across
/// platforms.
///
/// `Iso8601::DEFAULT` (what the `time` crate emits via `.format()`)
/// uses nanosecond precision and `+00:00:00` offset, which is valid
/// ISO 8601 but inconsistent with the schema doc-comment example
/// and harder for downstream consumers to pin against.
///
/// Input is converted to UTC before formatting, so a non-UTC clock
/// (only possible via tests) still produces a `Z`-suffixed string
/// representing the same instant.
pub fn format_iso8601_ms(t: OffsetDateTime) -> String {
    let utc = t.to_offset(time::UtcOffset::UTC);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        utc.year(),
        u8::from(utc.month()),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second(),
        utc.millisecond()
    )
}

pub fn session_path(env: &dyn Env) -> Option<PathBuf> {
    paths::config_dir(env).map(|d| d.join(SESSION_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;
    use time::format_description::well_known::Iso8601;

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    fn sample_actor() -> AgentActor {
        AgentActor {
            actor_type: "agent".to_string(),
            agent_id: "claude".to_string(),
            model: Some("claude-opus-4-7".to_string()),
            provider: Some("anthropic".to_string()),
            session_id: Some("sess-abc123".to_string()),
            thinking_effort: Some("high".to_string()),
        }
    }

    fn minimal_actor() -> AgentActor {
        AgentActor {
            actor_type: "agent".to_string(),
            agent_id: "github-actions".to_string(),
            model: None,
            provider: None,
            session_id: None,
            thinking_effort: None,
        }
    }

    fn sample_session(actor: AgentActor) -> SessionFile {
        SessionFile {
            version: SESSION_VERSION,
            actor,
            started_at: "2026-05-21T10:00:00.000Z".to_string(),
            pid: 12345,
            max_age_hours: 24,
        }
    }

    // ─── read / write round-trip ────────────────────────────────────────────

    #[test]
    fn read_returns_none_when_file_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let result = read(&env).expect("missing-file should resolve to None");
        assert!(
            result.is_none(),
            "missing file must be None, got {result:?}"
        );
    }

    #[test]
    fn write_then_read_round_trips_full_actor() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let sf = sample_session(sample_actor());
        write(&env, &sf).expect("write");
        let read_back = read(&env).expect("read").expect("file present after write");
        assert_eq!(read_back, sf);
    }

    #[test]
    fn write_then_read_round_trips_minimal_actor() {
        // Pins that optional fields stay omitted from the file (and on
        // re-read) rather than being persisted as `null` or sentinel
        // strings.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let sf = sample_session(minimal_actor());
        write(&env, &sf).expect("write");
        let read_back = read(&env).expect("read").expect("file present after write");
        assert_eq!(read_back, sf);
        assert!(read_back.actor.model.is_none());
        assert!(read_back.actor.provider.is_none());
    }

    #[test]
    fn write_omits_optional_actor_fields_from_serialised_json() {
        // Wire-format pin: omitted Option fields MUST NOT serialise as
        // `null` keys and MUST NOT serialise as sentinel strings. We
        // walk the parsed JSON tree rather than substring-match on the
        // raw text — substring matches would false-positive if any
        // value happened to contain "null" or "model" as a fragment.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let sf = sample_session(minimal_actor());
        write(&env, &sf).expect("write");

        let path = session_path(&env).expect("path resolves");
        let raw = std::fs::read_to_string(&path).expect("file exists");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("valid JSON");

        assert!(
            !contains_null_value(&parsed),
            "no Value::Null anywhere; got {raw}",
        );
        let actor = &parsed["actor"];
        assert!(actor.get("model").is_none(), "model omitted; got {raw}");
        assert!(
            actor.get("provider").is_none(),
            "provider omitted; got {raw}",
        );
        assert!(actor.get("thinkingEffort").is_none());
        assert!(actor.get("sessionId").is_none());

        // `"n/a"` is a forbidden literal — substring is sound here
        // because we never legitimately emit that string anywhere.
        assert!(
            !raw.contains("\"n/a\""),
            "no n/a sentinel anywhere; got {raw}",
        );
    }

    fn contains_null_value(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Null => true,
            serde_json::Value::Object(m) => m.values().any(contains_null_value),
            serde_json::Value::Array(a) => a.iter().any(contains_null_value),
            _ => false,
        }
    }

    #[test]
    fn write_creates_config_directory_if_missing() {
        // `<config-dir>` may not exist yet on a fresh install. `write`
        // creates the parent directory rather than erroring.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        // No `.notickets/` dir created yet.
        assert!(!tmp.path().join(".notickets").exists());
        write(&env, &sample_session(sample_actor())).expect("write");
        assert!(tmp.path().join(".notickets").exists());
    }

    #[test]
    fn second_write_replaces_first_atomically() {
        // Concurrent-replace property: a second `start` overwrites the
        // first cleanly via rename. We exercise the sequential case (the
        // atomicity of rename is the OS contract).
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let first = sample_session(sample_actor());
        let mut second = first.clone();
        second.actor.agent_id = "codex".to_string();

        write(&env, &first).expect("first write");
        write(&env, &second).expect("second write");
        let read_back = read(&env).expect("read").expect("present");
        assert_eq!(read_back.actor.agent_id, "codex");
    }

    #[test]
    fn write_does_not_leave_tmp_files_around() {
        // The PID+nanos tmp file lives in the same directory as the
        // destination so rename is atomic on POSIX. After a successful
        // write, no `*.tmp.*` leftover should remain.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        write(&env, &sample_session(sample_actor())).expect("write");
        let dir = tmp.path().join(".notickets");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                !name.contains(".tmp."),
                "tmp leftover in config dir: {name}",
            );
        }
    }

    // ─── delete ────────────────────────────────────────────────────────────

    #[test]
    fn delete_removes_session_file_when_present() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        write(&env, &sample_session(sample_actor())).expect("write");
        assert!(read(&env).unwrap().is_some(), "precondition: file present");

        delete(&env).expect("delete");
        assert!(read(&env).unwrap().is_none(), "file removed");
    }

    #[test]
    fn delete_is_idempotent_when_file_absent() {
        // No active session yet → delete must be a no-op success, not an
        // error. `session end` semantics.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        delete(&env).expect("delete on absent file is a no-op");
    }

    // ─── is_expired ─────────────────────────────────────────────────────────

    fn dt(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Iso8601::DEFAULT).expect("parse fixture")
    }

    #[test]
    fn is_expired_false_within_window() {
        let started = "2026-05-21T10:00:00.000Z";
        let now = dt("2026-05-21T20:00:00.000Z"); // +10h, well under 24h
        assert!(!is_expired(started, 24, now));
    }

    #[test]
    fn is_expired_true_past_window() {
        let started = "2026-05-21T10:00:00.000Z";
        let now = dt("2026-05-22T11:00:00.000Z"); // +25h, past 24h
        assert!(is_expired(started, 24, now));
    }

    #[test]
    fn is_expired_false_at_exact_boundary() {
        // At startedAt + maxAgeHours exactly, the session is still valid.
        // Strict-greater semantics — the boundary itself is in-window.
        let started = "2026-05-21T10:00:00.000Z";
        let now = dt("2026-05-22T10:00:00.000Z"); // +24h exact
        assert!(!is_expired(started, 24, now));
    }

    #[test]
    fn is_expired_honours_custom_max_age() {
        let started = "2026-05-21T10:00:00.000Z";
        let now = dt("2026-05-21T11:30:00.000Z"); // +1.5h
        assert!(!is_expired(started, 2, now));
        assert!(is_expired(started, 1, now));
    }

    #[test]
    fn is_expired_true_on_malformed_started_at() {
        // Garbage `startedAt` → defensively treat as expired so the
        // resolver falls through to the credentials branch instead of
        // emitting a corrupt actor.
        let now = dt("2026-05-21T20:00:00.000Z");
        assert!(is_expired("not-a-date", 24, now));
        assert!(is_expired("", 24, now));
    }

    // ─── session_path ───────────────────────────────────────────────────────

    #[test]
    fn session_path_lives_inside_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with_home(tmp.path());
        let p = session_path(&env).expect("path resolves");
        assert_eq!(p, tmp.path().join(".notickets").join(SESSION_FILE));
    }

    #[test]
    fn session_file_filename_pin() {
        // Pins the on-disk filename — changing this is a breaking change
        // for any agent harness that already wrote a session file.
        assert_eq!(SESSION_FILE, "active-session.json");
    }

    // ─── SessionError Display ───────────────────────────────────────────────

    #[test]
    fn session_error_display_home_unresolvable_names_the_problem() {
        // Pins the user-facing message. Without this, a mutant could
        // silently empty the Display impl and we'd lose diagnostic value
        // — `eprintln!("{e}")` in run_show/run_end would print nothing.
        let s = format!("{}", SessionError::HomeUnresolvable);
        assert!(
            s.contains("config directory"),
            "must mention config directory; got {s:?}",
        );
    }

    #[test]
    fn session_error_display_io_includes_inner_error() {
        let inner = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "boom");
        let s = format!("{}", SessionError::Io(inner));
        assert!(
            s.contains("active-session.json"),
            "must mention the file; got {s:?}",
        );
        assert!(
            s.contains("boom"),
            "must surface the underlying io error; got {s:?}",
        );
    }

    #[test]
    fn session_error_display_json_includes_parse_failure() {
        let inner = serde_json::from_str::<SessionFile>("not json").unwrap_err();
        let s = format!("{}", SessionError::Json(inner));
        assert!(
            s.contains("active-session.json"),
            "must mention the file; got {s:?}",
        );
        assert!(s.contains("parse"), "must say `parse`; got {s:?}");
    }

    // ─── format_iso8601_ms ──────────────────────────────────────────────────

    #[test]
    fn format_iso8601_ms_emits_millisecond_z_form() {
        // Pin the canonical wire shape: 4-digit year, 2-digit each of
        // month/day/hour/minute/second, 3-digit subsecond, literal `Z`.
        let t = OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::May, 21).unwrap(),
            time::Time::from_hms_milli(10, 0, 0, 123).unwrap(),
        );
        assert_eq!(format_iso8601_ms(t), "2026-05-21T10:00:00.123Z");
    }

    #[test]
    fn format_iso8601_ms_pads_single_digit_components() {
        // January, day 3, 04:05:06.007 — every field exercises padding.
        let t = OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::January, 3).unwrap(),
            time::Time::from_hms_milli(4, 5, 6, 7).unwrap(),
        );
        assert_eq!(format_iso8601_ms(t), "2026-01-03T04:05:06.007Z");
    }

    #[test]
    fn format_iso8601_ms_truncates_to_milliseconds() {
        // Anything finer than ms must not leak through.
        let t = OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::May, 21).unwrap(),
            time::Time::from_hms_nano(10, 0, 0, 999_888_777).unwrap(),
        );
        // 999_888_777 ns = 999 ms 888 us 777 ns → ".999"
        assert_eq!(format_iso8601_ms(t), "2026-05-21T10:00:00.999Z");
    }

    #[test]
    fn format_iso8601_ms_normalises_non_utc_input_to_z() {
        // A non-UTC clock (test-only scenario) must still emit a `Z`-
        // suffixed string at the equivalent UTC instant.
        let utc_dt = OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::May, 21).unwrap(),
            time::Time::from_hms_milli(10, 0, 0, 0).unwrap(),
        );
        let plus_two = utc_dt.to_offset(time::UtcOffset::from_hms(2, 0, 0).unwrap());
        assert_eq!(format_iso8601_ms(plus_two), "2026-05-21T10:00:00.000Z");
    }

    #[test]
    fn format_iso8601_ms_output_round_trips_through_iso8601_parse() {
        // is_expired() uses Iso8601::DEFAULT to parse `startedAt`. Pin
        // that anything format_iso8601_ms produces is parseable by the
        // same path — emitter and parser must agree.
        let t = OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::May, 21).unwrap(),
            time::Time::from_hms_milli(10, 0, 0, 123).unwrap(),
        );
        let s = format_iso8601_ms(t);
        let parsed = OffsetDateTime::parse(&s, &Iso8601::DEFAULT).expect("parseable");
        assert_eq!(parsed, t);
    }
}
