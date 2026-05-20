//! `nt publish` — single-event publish over HTTPS with Bearer auth.
//! Mirrors `src/cli/commands/publish` + `src/transport/events.ts::publish`.
//!
//! Scope: single event with bounded retry/backoff on transient failures
//! (Task 17 — retry policy + classifier live in `transport`) and an
//! opt-in machine-hash source attribute (Task 18 — computed in
//! `source_detect`, gated on `NO_TICKETS_INCLUDE_MACHINE=1`). `--stream`
//! mode (Task 4b) and batch mode (Task 16) live in their own tasks.

mod envelope;
mod metadata;
mod post;

use serde_json::Value;

use crate::auth::{resolve_auth, AuthOutcome, NOT_AUTH_MSG};
use crate::env::Env;
use crate::error::NtError;
use crate::source_detect::machine_hash_attribute;
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
}

pub async fn run(args: PublishArgs<'_>, env: &dyn Env) -> Result<(), NtError> {
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
    let meta = build_metadata(&args, machine_hash_owned.as_deref())
        .map_err(|message| NtError::Usage { message })?;

    let urls = resolve_urls(env).map_err(|e| NtError::Usage {
        message: e.user_message(),
    })?;

    let auth = match resolve_auth(env, &urls.api_url) {
        AuthOutcome::Resolved(a) => a,
        AuthOutcome::SessionHostMismatch {
            stored_host,
            current_host,
        } => {
            // Stored / current hosts go into dedicated fields on the
            // structured payload so wrappers can build their own
            // reconnect prompt without parsing `message` (which the
            // contract reserves for human display). `message` itself
            // gives a TTY user a useful summary.
            return Err(NtError::NotAuthenticated {
                message: format!(
                    "{NOT_AUTH_MSG} (stored session host {stored_host:?} \
                     does not match current host {current_host:?})"
                ),
                stored_host: Some(stored_host),
                current_host: Some(current_host),
            });
        }
        AuthOutcome::None => {
            return Err(NtError::NotAuthenticated {
                message: NOT_AUTH_MSG.to_string(),
                stored_host: None,
                current_host: None,
            });
        }
    };

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

    let client = Client::new(urls.api_url, auth.token).map_err(|e| NtError::Usage {
        message: e.to_string(),
    })?;

    // Edge done. Delegate to the testable core.
    let policy = RetryPolicy::default_publish();
    let sleeper = TokioSleeper;
    publish_event(&client, &policy, &sleeper, args.type_id, &parsed_data, meta).await
}
