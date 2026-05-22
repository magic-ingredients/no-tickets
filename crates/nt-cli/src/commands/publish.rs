//! `nt publish` — single-event publish over HTTPS with Bearer auth.
//!
//! Scope: single event with bounded retry/backoff on transient failures
//! (Task 17 — retry policy + classifier live in `transport`) and an
//! opt-in machine-hash source attribute (Task 18 — computed in
//! `source_detect`, gated on `NO_TICKETS_INCLUDE_MACHINE=1`). Batch mode
//! lives in `publish_batch`.
//!
//! Auth model: the Bearer token comes from
//! `auth::resolve_publish_token(env, --project)` — either the
//! `NO_TICKETS_TOKEN` env-var escape hatch or the push token registered
//! for the project in `config.json`. Session credentials from `nt init`
//! are deliberately not consulted by this path; they're a management-
//! API identity, not a publish credential. See
//! `docs/fixes/publish-uses-push-token.md`.

mod envelope;
mod metadata;
mod post;

use serde_json::Value;

// Re-export so the sibling batch command can share the structured-
// error mapping rather than emitting unstructured eprintln on
// transport failures. See `publish_batch::publish_envelopes`.
pub(crate) use post::map_transport_error;

use crate::actor::{self, ActorFlags};
use crate::auth::resolve_publish_token;
use crate::clock::Clock;
use crate::env::Env;
use crate::error::NtError;
use crate::hint;
use crate::source_detect::machine_hash_attribute;
use crate::state;
use crate::transport::{Client, RetryPolicy, TokioSleeper};
use crate::urls::resolve_urls;

use metadata::build_metadata;
use post::publish_event;

/// Thin funnel for `publish_batch::source::build_cli_base_source`, which shares
/// the `--source-attribute` parser with the single-event path. Drift
/// here would silently re-attribute every batch event but not single
/// events (or vice versa). Wrapping (rather than `pub(super) use`)
/// keeps `metadata::parse_source_attribute` itself at `pub(super)` —
/// visible only inside the `publish` module — and forces `publish_batch`
/// to come in through the `publish` module's public surface.
pub(super) fn parse_source_attribute(raw: &str) -> Result<(&str, &str), String> {
    metadata::parse_source_attribute(raw)
}

/// Default `source.name` when no `--source-name` flag is supplied. Shared
/// with `publish_batch` to keep single-event and batch paths in lockstep
/// — a drift here would silently re-attribute every event from one
/// surface but not the other.
pub(super) const DEFAULT_SOURCE_NAME: &str = "no-tickets-cli";

/// SDK version stamped into every envelope's `source.sdkVersion`. Bound
/// to the binary's own crate version at compile time so a binary
/// release and the attribution it produces never disagree. Shared with
/// `publish_batch` for the same single-source-of-truth reason.
pub(super) const SDK_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct PublishArgs<'a> {
    pub type_id: &'a str,
    /// Raw `--data` argument. Parsed inside `run()` so the i32 exit-code
    /// contract owns the full input-handling surface (main.rs is
    /// dispatch-only; doesn't short-circuit with its own exit calls).
    pub data: &'a str,
    pub project: &'a str,
    pub source_name: Option<&'a str>,
    /// Raw `--source-attribute KEY=VALUE` repeats. Parsed inside `run()`
    /// so usage errors flow through the same exit-1 path as the rest of
    /// the input validation.
    pub source_attributes: &'a [String],
    pub parent: Option<&'a str>,
    pub trace: Option<&'a str>,
    pub dedupe_key: Option<&'a str>,
    /// Actor-resolution inputs sourced from the `--actor-*` /
    /// `--call-id` / `--prompt-tokens` / etc. flag family. The resolver
    /// (`actor::resolve`) combines them with session and credentials
    /// state to produce the optional `metadata.actor` block.
    pub actor: ActorFlags<'a>,
    /// `--quiet` — suppresses the first-publish hint on stderr. Does
    /// NOT suppress the marker write to `state.json` (per PRD).
    pub quiet: bool,
}

pub async fn run(args: PublishArgs<'_>, env: &dyn Env, clock: &dyn Clock) -> Result<(), NtError> {
    // Compute the optional machine-hash attribute first so its String
    // borrow has a stable run-local lifetime that `build_metadata` can
    // weave into the attributes BTreeMap alongside flag-derived
    // entries. None means env-var off OR best-effort FS failure;
    // either way, no attribute on the wire.
    let machine_hash_owned: Option<String> = machine_hash_attribute(env);

    // Usage validation FIRST — before any auth/network/file-system
    // resolution — so a bad flag combo doesn't leak credentials state
    // or surface a confusing "not authenticated" message when the real
    // fault is a malformed argv.
    let mut meta = build_metadata(&args, machine_hash_owned.as_deref())
        .map_err(|message| NtError::Usage { message })?;

    let urls = resolve_urls(env).map_err(|e| NtError::Usage {
        message: e.user_message(),
    })?;

    // Resolve the publish token from the project registry (or the
    // env-var escape hatch). NEVER session credentials — see module
    // docstring.
    let token = resolve_publish_token(env, args.project)?;

    // Opt-in actor attribution. resolve() walks the precedence chain
    // exactly once per invocation. The hint mechanic runs AFTER a
    // successful publish — emitting before would mix the hint line
    // with the structured-error JSON on stderr when the publish itself
    // fails (auth / 4xx / 5xx). Session-attributed branches (1–4)
    // short-circuit `fire_hint_if_needed` before any state.json IO.
    let resolved = actor::resolve(env, clock, &args.actor, &urls.api_url);
    meta.actor = resolved.actor.clone();

    // --data must be valid JSON. Parsing inside run() means the contract
    // owns the full input-handling path. Local *schema* validation is
    // intentionally NOT done here — that's the dedicated `nt validate`
    // command's job; `nt publish` ships to the server and surfaces the
    // server's verdict via the transport-error mapping. Adding local
    // pre-flight here would silently expand publish's contract beyond
    // what Task 26 is scoped to do.
    let parsed_data: Value = serde_json::from_str(args.data).map_err(|e| NtError::Usage {
        message: format!("--data must be valid JSON: {e}"),
    })?;

    let client = Client::new(urls.api_url, token).map_err(|e| NtError::Usage {
        message: e.to_string(),
    })?;

    // Edge done. Delegate to the testable core.
    let policy = RetryPolicy::default_publish();
    let sleeper = TokioSleeper;
    publish_event(&client, &policy, &sleeper, args.type_id, &parsed_data, meta).await?;
    // First-publish hint runs only on the success path. Failures route
    // through the structured-error contract (single-line JSON on
    // stderr); mixing the hint into that surface would break parsers.
    fire_hint_if_needed(env, &resolved, args.quiet);
    Ok(())
}

/// Run the first-publish hint mechanic. On the no-actor branch with the
/// marker unset, prints the hint to stderr (unless `quiet`) and
/// atomically updates `state.json` to set the marker. On any branch
/// with a resolved actor, performs ZERO `state.json` IO — pinned by
/// the integration tests in `tests/actor-resolution.rs`.
fn fire_hint_if_needed(env: &dyn Env, resolved: &actor::Resolved, quiet_flag: bool) {
    // Short-circuit: session-attributed paths do not touch state.json.
    if resolved.actor.is_some() {
        return;
    }
    // NO_TICKETS_QUIET=anything-non-empty also suppresses stderr (per
    // PRD), so the operator doesn't have to keep the flag set forever
    // after an initial opt-out. The flag remains authoritative when set.
    let env_quiet = env.var("NO_TICKETS_QUIET").is_some_and(|v| !v.is_empty());
    let quiet = quiet_flag || env_quiet;

    let current = state::read(env).ok().flatten();
    let decision = hint::decide(resolved, current.as_ref(), quiet);
    if decision.emit_stderr {
        eprintln!("{}", hint::render());
    }
    if decision.set_marker {
        let mut new_state = current.unwrap_or_default();
        new_state.first_publish_hint_shown = true;
        // Best-effort: a failed marker write would re-fire the hint
        // next invocation, which is more annoying than a silent
        // failure — but the publish itself already succeeded, so a
        // hard error here would mis-classify the outcome.
        let _ = state::write(env, &new_state);
    }
}
