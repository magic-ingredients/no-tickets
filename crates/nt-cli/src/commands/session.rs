//! `no-tickets session start | show | end` — agent-harness identity
//! lifecycle. Writes `<config-dir>/active-session.json` so subsequent
//! `no-tickets publish` invocations can stamp `metadata.actor`.
//!
//! `start` requires only `--agent`. Other LLM-context flags are optional
//! and omitted from the actor block (no `"n/a"` sentinels) when not
//! supplied.
//!
//! `show` prints the active session as a JSON object including an
//! `expired` flag, or `{"active":false}` when no session is set.
//!
//! `end` deletes the session file **and** clears
//! `firstPublishHintShown` from `state.json` (idempotent — both
//! operations succeed when the target is already absent).

use serde_json::json;
use time::format_description::well_known::Iso8601;

use crate::clock::Clock;
use crate::env::Env;
use crate::session::{self, AgentActor, SessionFile, SESSION_VERSION};
use crate::state;

pub struct StartArgs<'a> {
    pub agent: &'a str,
    pub model: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub thinking_effort: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub max_age_hours: u32,
}

pub fn run_start(env: &dyn Env, clock: &dyn Clock, args: StartArgs<'_>) -> i32 {
    let actor = AgentActor {
        actor_type: "agent".to_string(),
        agent_id: args.agent.to_string(),
        model: args.model.map(str::to_string),
        provider: args.provider.map(str::to_string),
        session_id: args.session_id.map(str::to_string),
        thinking_effort: args.thinking_effort.map(str::to_string),
    };
    let started_at = clock
        .now()
        .format(&Iso8601::DEFAULT)
        .unwrap_or_else(|_| "1970-01-01T00:00:00.000000000Z".to_string());
    let sf = SessionFile {
        version: SESSION_VERSION,
        actor,
        started_at,
        pid: std::process::id(),
        max_age_hours: args.max_age_hours,
    };
    if let Err(e) = session::write(env, &sf) {
        eprintln!("{e}");
        return 1;
    }
    0
}

pub fn run_show(env: &dyn Env, clock: &dyn Clock) -> i32 {
    let sf = match session::read(env) {
        Ok(Some(sf)) => sf,
        Ok(None) => {
            println!("{}", json!({ "active": false }));
            return 0;
        }
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let expired = session::is_expired(&sf.started_at, sf.max_age_hours, clock.now());
    let out = json!({
        "active": true,
        "actor": sf.actor,
        "startedAt": sf.started_at,
        "pid": sf.pid,
        "maxAgeHours": sf.max_age_hours,
        "expired": expired,
    });
    println!("{out}");
    0
}

pub fn run_end(env: &dyn Env) -> i32 {
    if let Err(e) = session::delete(env) {
        eprintln!("{e}");
        return 1;
    }
    if let Err(e) = state::clear_hint_marker(env) {
        eprintln!("{e}");
        return 1;
    }
    0
}
