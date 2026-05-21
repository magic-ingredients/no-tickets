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

use crate::clock::Clock;
use crate::env::Env;

#[allow(dead_code)] // fields wired into the session file in GREEN
pub struct StartArgs<'a> {
    pub agent: &'a str,
    pub model: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub thinking_effort: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub max_age_hours: u32,
}

#[allow(dead_code, unused_variables)]
pub fn run_start(env: &dyn Env, clock: &dyn Clock, args: StartArgs<'_>) -> i32 {
    // RED stub: claims success without writing the session file.
    0
}

#[allow(dead_code, unused_variables)]
pub fn run_show(env: &dyn Env, clock: &dyn Clock) -> i32 {
    // RED stub: prints nothing.
    0
}

#[allow(dead_code, unused_variables)]
pub fn run_end(env: &dyn Env) -> i32 {
    // RED stub.
    0
}
