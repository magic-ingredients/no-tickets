//! First-publish hint mechanic for `no-tickets publish`.
//!
//! Fires at most once per `<config-dir>/state.json` lifetime: when an
//! unattributed publish lands and `state.json` does not have
//! `firstPublishHintShown: true`, the CLI prints a one-time hint to
//! stderr and sets the marker. `--quiet` (or `NO_TICKETS_QUIET=1`)
//! suppresses the stderr output but still sets the marker so the env-
//! var doesn't have to stay set forever.
//!
//! The marker is cleared by `no-tickets session end`, so a future
//! unattributed publish can re-fire the hint.
//!
//! This module is **pure**: `decide` reads no I/O and writes no state;
//! `render` returns a static template. Callers do the marker write and
//! the stderr emit.

use crate::actor::Resolved;
use crate::state::State;

/// What the publish flow should do about the hint, given the resolved
/// actor and the current marker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HintDecision {
    /// Whether to print the hint text to stderr. Always `false` when
    /// `quiet` is `true`, regardless of whether the marker is set.
    pub emit_stderr: bool,
    /// Whether to atomically write the marker flag to `state.json`.
    /// True iff the resolved actor is `None` AND the marker is unset —
    /// `--quiet` does NOT suppress the marker write.
    pub set_marker: bool,
}

#[allow(dead_code, unused_variables)] // wired by GREEN
pub fn decide(resolved: &Resolved, state: Option<&State>, quiet: bool) -> HintDecision {
    // RED stub: never fires, never sets.
    HintDecision {
        emit_stderr: false,
        set_marker: false,
    }
}

/// User-facing hint text. Static — no caller info or env-sniffed
/// agent name (the PRD calls out that the hint is "deliberately
/// generic; declaration is the caller's explicit choice").
#[allow(dead_code)] // wired by GREEN
pub fn render() -> &'static str {
    // RED stub: empty.
    ""
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::{ResolutionSource, Resolved};
    use crate::session::AgentActor;

    fn unattributed() -> Resolved {
        Resolved {
            actor: None,
            source: ResolutionSource::Unattributed,
        }
    }

    fn attributed() -> Resolved {
        Resolved {
            actor: Some(crate::actor::Actor::Agent(AgentActor {
                actor_type: "agent".to_string(),
                agent_id: "claude".to_string(),
                model: None,
                provider: None,
                session_id: None,
                thinking_effort: None,
                call_id: None,
                prompt_tokens: None,
                completion_tokens: None,
                latency_ms: None,
            })),
            source: ResolutionSource::ActiveSessionFile,
        }
    }

    fn marker_unset() -> State {
        State {
            first_publish_hint_shown: false,
            extras: serde_json::Map::new(),
        }
    }

    fn marker_set() -> State {
        State {
            first_publish_hint_shown: true,
            extras: serde_json::Map::new(),
        }
    }

    // ─── decide() ──────────────────────────────────────────────────────────

    #[test]
    fn decide_fires_when_unattributed_and_marker_unset_and_not_quiet() {
        let d = decide(&unattributed(), Some(&marker_unset()), false);
        assert!(d.emit_stderr, "first unattributed publish must hint");
        assert!(d.set_marker, "marker must be persisted so we don't repeat");
    }

    #[test]
    fn decide_fires_when_unattributed_and_no_state_file_present() {
        // `state.json` absent → treat marker as unset → fire.
        let d = decide(&unattributed(), None, false);
        assert!(d.emit_stderr, "absent state.json must not suppress hint");
        assert!(d.set_marker, "absent state.json must still set marker");
    }

    #[test]
    fn decide_does_not_fire_when_marker_already_set() {
        let d = decide(&unattributed(), Some(&marker_set()), false);
        assert!(!d.emit_stderr, "marker set → no repeat hint on stderr",);
        assert!(!d.set_marker, "no need to re-write the marker — idempotent",);
    }

    #[test]
    fn decide_suppresses_stderr_under_quiet_but_still_sets_marker() {
        // Critical contract: --quiet silences the stderr text but the
        // marker is still set so the hint doesn't fire on subsequent
        // publishes once --quiet is dropped.
        let d = decide(&unattributed(), Some(&marker_unset()), true);
        assert!(!d.emit_stderr, "quiet must suppress stderr");
        assert!(
            d.set_marker,
            "quiet must NOT suppress marker write — see PRD",
        );
    }

    #[test]
    fn decide_does_not_fire_when_actor_is_resolved() {
        // Session-attributed paths: no hint, no marker write, regardless
        // of marker state or quiet flag.
        for quiet in [false, true] {
            for state in [None, Some(&marker_unset()), Some(&marker_set())] {
                let d = decide(&attributed(), state, quiet);
                assert!(
                    !d.emit_stderr,
                    "attributed publish must not emit hint (quiet={quiet}, state.set={:?})",
                    state.map(|s| s.first_publish_hint_shown),
                );
                assert!(
                    !d.set_marker,
                    "attributed publish must not touch marker (quiet={quiet})",
                );
            }
        }
    }

    // ─── render() ──────────────────────────────────────────────────────────

    #[test]
    fn render_mentions_session_start_command() {
        let text = render();
        assert!(
            text.contains("no-tickets session start"),
            "hint must surface the recovery command; got {text:?}",
        );
    }

    #[test]
    fn render_mentions_session_end_clears_marker() {
        let text = render();
        assert!(
            text.contains("session end"),
            "hint must explain how the marker is cleared; got {text:?}",
        );
    }

    #[test]
    fn render_does_not_mention_environment_sniffing_agent_ids() {
        // PRD: hint is deliberately generic. Must NOT pre-fill
        // a guessed agent id from env vars like CLAUDECODE.
        let text = render();
        assert!(
            !text.to_lowercase().contains("claudecode"),
            "no harness-env name in the hint; got {text:?}",
        );
        assert!(
            !text.to_lowercase().contains("github_actions"),
            "no harness-env name in the hint; got {text:?}",
        );
    }
}
