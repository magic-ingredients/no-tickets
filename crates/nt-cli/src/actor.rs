//! Actor resolution for `no-tickets publish`.
//!
//! Per the event-actor-metadata PRD, attribution is **opt-in**: callers
//! who want their events attributed declare an identity via flags or a
//! `no-tickets session`, and publish stamps `metadata.actor` on the
//! envelope. Callers who don't opt in publish unattributed events —
//! `metadata` is omitted from the wire and the database value is NULL.
//!
//! Resolution precedence (first wins):
//!   1. `--agent-id` flag present                              → flags
//!   2. `NO_TICKETS_SESSION_FILE` env var → read that file     → session-env
//!   3. `<config-dir>/active-session.json` present + fresh     → session-file
//!   4. session credentials present (`no-tickets init` was run) → credentials
//!   5. otherwise                                              → unattributed
//!
//! Per-call enrichment flags (`--call-id`, `--prompt-tokens`,
//! `--completion-tokens`, `--latency-ms`) are **layered on top** of
//! whatever the precedence chain resolves — they're never identity
//! fields by themselves.

use crate::clock::Clock;
use crate::env::Env;
use crate::session::AgentActor;

/// Variant of the wire actor block produced by `no-tickets init`-derived
/// human credentials. `userId` is the only mandatory field; `email` is
/// optional (per the canonical schema).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct HumanActor {
    #[serde(rename = "type")]
    pub actor_type: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// Discriminated union of actor variants. Serialised as the inner
/// struct's own shape — both variants carry their own `type`
/// discriminator field, so `untagged` is faithful to the wire.
#[allow(dead_code)] // variants constructed by GREEN resolver
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(untagged)]
pub enum Actor {
    Agent(AgentActor),
    Human(HumanActor),
}

/// Wire-shape envelope-level metadata block. Lives between `data` and
/// `source` in `EventEnvelope`. `metadata` is optional; when no actor
/// resolves the entire `metadata` key is omitted from the wire (not
/// emitted as `null`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EventMetadata {
    pub actor: Actor,
}

/// Which branch of the precedence chain produced the resolved actor.
/// Used by `hint::decide` to gate the first-publish hint.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // variants populated by GREEN
pub enum ResolutionSource {
    /// Resolved entirely from `--actor-*` flags.
    Flags,
    /// Read from the path in `NO_TICKETS_SESSION_FILE`.
    SessionFileEnv,
    /// Read from `<config-dir>/active-session.json`.
    ActiveSessionFile,
    /// Built from session credentials (`no-tickets init` output).
    Credentials,
    /// No identity declared.
    Unattributed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub actor: Option<Actor>,
    pub source: ResolutionSource,
}

/// Subset of `nt publish` CLI flags that feed actor resolution.
///
/// `actor_type` accepts `"human"` or `"agent"`; in practice flag-driven
/// resolution always builds the agent variant (the flag set has no
/// `--user-id` — human actors come from credentials, not flags).
#[allow(dead_code)] // fields consumed by GREEN resolver
#[derive(Debug, Default, Clone)]
pub struct ActorFlags<'a> {
    pub actor_type: Option<&'a str>,
    pub agent_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub provider: Option<&'a str>,
    pub thinking_effort: Option<&'a str>,
    pub session_id: Option<&'a str>,
    pub call_id: Option<&'a str>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
    pub latency_ms: Option<u64>,
    pub session_file: Option<&'a str>,
}

/// Resolve the actor for a single `no-tickets publish` invocation.
///
/// Pure-ish: takes injected `env` (env-var read port), `clock` (for
/// staleness checks on the session file), the parsed flags, and the
/// already-resolved `api_url` (used by the credentials branch to
/// host-check the stored credentials).
#[allow(dead_code, unused_variables)] // wired by GREEN
pub fn resolve(
    env: &dyn Env,
    clock: &dyn Clock,
    flags: &ActorFlags<'_>,
    api_url: &str,
) -> Resolved {
    // RED stub: always unattributed.
    Resolved {
        actor: None,
        source: ResolutionSource::Unattributed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;
    use crate::env::HashMapEnv;
    use crate::session::{self, SessionFile, SESSION_VERSION};
    use std::path::Path;
    use time::format_description::well_known::Iso8601;
    use time::OffsetDateTime;

    // ─── helpers ────────────────────────────────────────────────────────────

    struct FixedClock(OffsetDateTime);
    impl Clock for FixedClock {
        fn now(&self) -> OffsetDateTime {
            self.0
        }
    }

    fn dt(s: &str) -> OffsetDateTime {
        OffsetDateTime::parse(s, &Iso8601::DEFAULT).expect("parse fixture")
    }

    fn env_with(home: &Path, extras: &[(&str, &str)]) -> HashMapEnv {
        let mut pairs: Vec<(&str, &str)> = vec![("NO_TICKETS_HOME", home.to_str().unwrap())];
        pairs.extend(extras.iter().copied());
        HashMapEnv::with(&pairs)
    }

    fn agent_actor(agent_id: &str) -> AgentActor {
        AgentActor {
            actor_type: "agent".to_string(),
            agent_id: agent_id.to_string(),
            model: None,
            provider: None,
            session_id: None,
            thinking_effort: None,
            call_id: None,
            prompt_tokens: None,
            completion_tokens: None,
            latency_ms: None,
        }
    }

    fn write_session(env: &dyn Env, started_at: &str, actor: AgentActor) {
        let sf = SessionFile {
            version: SESSION_VERSION,
            actor,
            started_at: started_at.to_string(),
            pid: 1,
            max_age_hours: 24,
        };
        session::write(env, &sf).expect("write session");
    }

    fn write_credentials(home: &Path, email: &str, expires_at: &str, host: &str) {
        let dir = home.join(".notickets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("credentials"),
            format!(
                r#"{{"token":"t","email":"{email}","expiresAt":"{expires_at}","host":"{host}"}}"#,
            ),
        )
        .unwrap();
    }

    const FAR_FUTURE: &str = "2099-01-01T00:00:00.000Z";
    const PROD_API: &str = "https://api.no-tickets.com";

    // ─── precedence: each branch in isolation ──────────────────────────────

    #[test]
    fn resolve_with_no_inputs_is_unattributed() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        let clock = FixedClock(dt("2026-05-21T10:00:00.000Z"));
        let flags = ActorFlags::default();

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::Unattributed);
        assert!(r.actor.is_none(), "no actor when nothing declared");
    }

    #[test]
    fn resolve_with_agent_id_flag_returns_flag_built_agent() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        let clock = FixedClock(dt("2026-05-21T10:00:00.000Z"));
        let flags = ActorFlags {
            actor_type: Some("agent"),
            agent_id: Some("claude"),
            model: Some("claude-opus-4-7"),
            ..Default::default()
        };

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::Flags);
        let Some(Actor::Agent(a)) = r.actor else {
            panic!("expected agent, got {:?}", r.actor);
        };
        assert_eq!(a.agent_id, "claude");
        assert_eq!(a.model.as_deref(), Some("claude-opus-4-7"));
    }

    #[test]
    fn resolve_with_session_file_env_var_reads_that_path() {
        // Branch 2: NO_TICKETS_SESSION_FILE points at an alternate path.
        // Must read that file rather than the default active-session.json.
        let tmp = tempfile::tempdir().unwrap();
        let alt_path = tmp.path().join("alt-session.json");
        let sf = SessionFile {
            version: SESSION_VERSION,
            actor: agent_actor("codex"),
            started_at: "2026-05-21T10:00:00.000Z".to_string(),
            pid: 1,
            max_age_hours: 24,
        };
        std::fs::write(&alt_path, serde_json::to_string(&sf).unwrap()).unwrap();

        let env = env_with(
            tmp.path(),
            &[("NO_TICKETS_SESSION_FILE", alt_path.to_str().unwrap())],
        );
        let clock = FixedClock(dt("2026-05-21T11:00:00.000Z")); // +1h, fresh
        let flags = ActorFlags::default();

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::SessionFileEnv);
        match r.actor {
            Some(Actor::Agent(a)) => assert_eq!(a.agent_id, "codex"),
            other => panic!("expected agent from alt file, got {other:?}"),
        }
    }

    #[test]
    fn resolve_with_active_session_file_returns_that_actor() {
        // Branch 3: <config-dir>/active-session.json present and fresh.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        let clock = FixedClock(dt("2026-05-21T11:00:00.000Z")); // +1h, fresh
        write_session(&env, "2026-05-21T10:00:00.000Z", agent_actor("claude"));
        let flags = ActorFlags::default();

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::ActiveSessionFile);
        match r.actor {
            Some(Actor::Agent(a)) => assert_eq!(a.agent_id, "claude"),
            other => panic!("expected agent from active-session.json, got {other:?}"),
        }
    }

    #[test]
    fn resolve_stale_session_file_falls_through_to_next_branch() {
        // Session present but `now > startedAt + maxAgeHours` → must NOT
        // emit a stale actor. With no credentials either, falls all the
        // way through to Unattributed.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        // Started 25h ago — past the 24h default window.
        let clock = FixedClock(dt("2026-05-22T11:01:00.000Z"));
        write_session(&env, "2026-05-21T10:00:00.000Z", agent_actor("claude"));
        let flags = ActorFlags::default();

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(
            r.source,
            ResolutionSource::Unattributed,
            "stale session must NOT resolve; expected fall-through",
        );
        assert!(r.actor.is_none());
    }

    #[test]
    fn resolve_with_credentials_returns_human_actor() {
        // Branch 4: no session, but session credentials exist + match
        // the api_url. Builds a HumanActor.
        let tmp = tempfile::tempdir().unwrap();
        write_credentials(tmp.path(), "alice@example.com", FAR_FUTURE, PROD_API);
        let env = env_with(tmp.path(), &[]);
        let clock = FixedClock(dt("2026-05-21T10:00:00.000Z"));
        let flags = ActorFlags::default();

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::Credentials);
        match r.actor {
            Some(Actor::Human(h)) => {
                assert!(
                    !h.user_id.is_empty(),
                    "human actor must carry a non-empty userId",
                );
                assert_eq!(h.email.as_deref(), Some("alice@example.com"));
            }
            other => panic!("expected human from credentials, got {other:?}"),
        }
    }

    // ─── precedence ordering: flags > env > file > credentials ─────────────

    #[test]
    fn resolve_flag_identity_wins_over_active_session() {
        // Both set — flags MUST win.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        write_session(&env, "2026-05-21T10:00:00.000Z", agent_actor("claude"));
        let clock = FixedClock(dt("2026-05-21T11:00:00.000Z"));
        let flags = ActorFlags {
            actor_type: Some("agent"),
            agent_id: Some("codex"),
            ..Default::default()
        };

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::Flags);
        match r.actor {
            Some(Actor::Agent(a)) => assert_eq!(a.agent_id, "codex"),
            other => panic!("flags must win; got {other:?}"),
        }
    }

    #[test]
    fn resolve_env_var_path_wins_over_active_session() {
        let tmp = tempfile::tempdir().unwrap();
        let alt_path = tmp.path().join("alt.json");
        let sf = SessionFile {
            version: SESSION_VERSION,
            actor: agent_actor("from-env"),
            started_at: "2026-05-21T10:00:00.000Z".to_string(),
            pid: 1,
            max_age_hours: 24,
        };
        std::fs::write(&alt_path, serde_json::to_string(&sf).unwrap()).unwrap();
        let env = env_with(
            tmp.path(),
            &[("NO_TICKETS_SESSION_FILE", alt_path.to_str().unwrap())],
        );
        write_session(
            &env,
            "2026-05-21T10:00:00.000Z",
            agent_actor("from-default"),
        );
        let clock = FixedClock(dt("2026-05-21T11:00:00.000Z"));

        let r = resolve(&env, &clock, &ActorFlags::default(), PROD_API);
        assert_eq!(r.source, ResolutionSource::SessionFileEnv);
        match r.actor {
            Some(Actor::Agent(a)) => assert_eq!(a.agent_id, "from-env"),
            other => panic!("env path must win over default; got {other:?}"),
        }
    }

    #[test]
    fn resolve_active_session_wins_over_credentials() {
        let tmp = tempfile::tempdir().unwrap();
        write_credentials(tmp.path(), "alice@example.com", FAR_FUTURE, PROD_API);
        let env = env_with(tmp.path(), &[]);
        write_session(&env, "2026-05-21T10:00:00.000Z", agent_actor("claude"));
        let clock = FixedClock(dt("2026-05-21T11:00:00.000Z"));

        let r = resolve(&env, &clock, &ActorFlags::default(), PROD_API);
        assert_eq!(r.source, ResolutionSource::ActiveSessionFile);
        match r.actor {
            Some(Actor::Agent(_)) => {}
            other => panic!("active session must beat credentials; got {other:?}"),
        }
    }

    // ─── per-call enrichment layering ──────────────────────────────────────

    #[test]
    fn resolve_layers_call_id_on_session_actor() {
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        write_session(&env, "2026-05-21T10:00:00.000Z", agent_actor("claude"));
        let clock = FixedClock(dt("2026-05-21T11:00:00.000Z"));
        let flags = ActorFlags {
            call_id: Some("call-xyz"),
            prompt_tokens: Some(1234),
            completion_tokens: Some(567),
            latency_ms: Some(812),
            ..Default::default()
        };

        let r = resolve(&env, &clock, &flags, PROD_API);
        match r.actor {
            Some(Actor::Agent(a)) => {
                assert_eq!(a.agent_id, "claude", "identity stays from session");
                assert_eq!(a.call_id.as_deref(), Some("call-xyz"));
                assert_eq!(a.prompt_tokens, Some(1234));
                assert_eq!(a.completion_tokens, Some(567));
                assert_eq!(a.latency_ms, Some(812));
            }
            other => panic!("expected layered agent, got {other:?}"),
        }
    }

    #[test]
    fn resolve_per_call_flags_alone_do_not_construct_an_actor() {
        // --call-id without --agent-id (or session/credentials) must NOT
        // synthesise an actor. Enrichment is overlay, not identity.
        let tmp = tempfile::tempdir().unwrap();
        let env = env_with(tmp.path(), &[]);
        let clock = FixedClock(dt("2026-05-21T10:00:00.000Z"));
        let flags = ActorFlags {
            call_id: Some("call-xyz"),
            ..Default::default()
        };

        let r = resolve(&env, &clock, &flags, PROD_API);
        assert_eq!(r.source, ResolutionSource::Unattributed);
        assert!(r.actor.is_none());
    }

    // ─── wire-shape of the resolved actor (serialisation) ──────────────────

    #[test]
    fn agent_actor_serialises_with_camelcase_keys_and_type_discriminator() {
        let a = AgentActor {
            actor_type: "agent".to_string(),
            agent_id: "claude".to_string(),
            model: Some("claude-opus-4-7".to_string()),
            provider: Some("anthropic".to_string()),
            session_id: Some("sess-1".to_string()),
            thinking_effort: Some("high".to_string()),
            call_id: Some("call-1".to_string()),
            prompt_tokens: Some(100),
            completion_tokens: Some(50),
            latency_ms: Some(250),
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(json["type"], "agent");
        assert_eq!(json["agentId"], "claude");
        assert_eq!(json["model"], "claude-opus-4-7");
        assert_eq!(json["provider"], "anthropic");
        assert_eq!(json["sessionId"], "sess-1");
        assert_eq!(json["thinkingEffort"], "high");
        assert_eq!(json["callId"], "call-1");
        assert_eq!(json["promptTokens"], 100);
        assert_eq!(json["completionTokens"], 50);
        assert_eq!(json["latencyMs"], 250);
    }

    #[test]
    fn human_actor_serialises_with_camelcase_keys_and_type_discriminator() {
        let h = HumanActor {
            actor_type: "human".to_string(),
            user_id: "u-1".to_string(),
            email: Some("alice@example.com".to_string()),
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(json["type"], "human");
        assert_eq!(json["userId"], "u-1");
        assert_eq!(json["email"], "alice@example.com");
    }

    #[test]
    fn human_actor_omits_email_when_none() {
        let h = HumanActor {
            actor_type: "human".to_string(),
            user_id: "u-1".to_string(),
            email: None,
        };
        let raw = serde_json::to_string(&h).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(
            json.get("email").is_none(),
            "email must be omitted when None; got {raw}",
        );
    }

    #[test]
    fn actor_enum_serialises_as_its_inner_struct_untagged() {
        // Untagged enum: the wire shape is the inner struct's shape, no
        // extra wrapping `{"Agent":{...}}` envelope.
        let agent = Actor::Agent(AgentActor {
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
        });
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&agent).unwrap()).unwrap();
        assert_eq!(json["type"], "agent");
        assert_eq!(json["agentId"], "claude");
        assert!(
            json.get("Agent").is_none(),
            "untagged: no `Agent` wrapper key",
        );
    }

    #[test]
    fn event_metadata_serialises_with_actor_key() {
        let meta = EventMetadata {
            actor: Actor::Agent(AgentActor {
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
            }),
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&meta).unwrap()).unwrap();
        assert_eq!(json["actor"]["type"], "agent");
        assert_eq!(json["actor"]["agentId"], "claude");
    }
}
