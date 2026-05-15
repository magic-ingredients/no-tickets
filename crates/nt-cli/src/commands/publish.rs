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

use crate::auth::{emit_host_mismatch_warning, resolve_auth, AuthOutcome, NOT_AUTH_MSG};
use crate::env::Env;
use crate::source_detect::machine_hash_attribute;
use crate::transport::{Client, RetryPolicy, TokioSleeper};
use crate::urls::resolve_urls;

use metadata::build_metadata;
use post::publish_event;

// Re-exported for `publish_batch::build_cli_base_source`, which shares
// the source-attribute parser + the default source.name + sdkVersion
// stamp with the single-event path. Drift here would silently
// re-attribute every batch event but not single events (or vice
// versa).
pub(super) use metadata::parse_source_attribute;

/// Default `source.name` when no `--source-name` flag is supplied. Shared
/// with `publish_batch` to keep single-event and batch paths in lockstep
/// — a drift here would silently re-attribute every event from one
/// surface but not the other.
pub(super) const DEFAULT_SOURCE_NAME: &str = "nt-cli";

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
    pub subject_type: Option<&'a str>,
    pub subject_id: Option<&'a str>,
    pub source_name: Option<&'a str>,
    /// Raw `--source-attribute KEY=VALUE` repeats. Parsed inside `run()`
    /// so usage errors flow through the same exit-1 path as the rest of
    /// the input validation.
    pub source_attributes: &'a [String],
    pub parent: Option<&'a str>,
    pub trace: Option<&'a str>,
    pub dedupe_key: Option<&'a str>,
}

pub async fn run(args: PublishArgs<'_>, env: &dyn Env) -> i32 {
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
    let meta = match build_metadata(&args, machine_hash_owned.as_deref()) {
        Ok(m) => m,
        Err(msg) => {
            eprintln!("{msg}");
            return 1;
        }
    };

    let urls = match resolve_urls(env) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let auth = match resolve_auth(env, &urls.api_url) {
        AuthOutcome::Resolved(a) => a,
        AuthOutcome::SessionHostMismatch {
            stored_host,
            current_host,
        } => {
            emit_host_mismatch_warning(&stored_host, &current_host);
            eprintln!("{NOT_AUTH_MSG}");
            return 1;
        }
        AuthOutcome::None => {
            eprintln!("{NOT_AUTH_MSG}");
            return 1;
        }
    };

    // --data must be valid JSON. Parsing inside run() means the i32
    // exit-code contract owns the full input-handling path.
    let parsed_data: Value = match serde_json::from_str(args.data) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("--data must be valid JSON: {e}");
            return 1;
        }
    };

    let client = match Client::new(urls.api_url, auth.token) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // Edge done. Delegate to the testable core.
    let policy = RetryPolicy::default_publish();
    let sleeper = TokioSleeper;
    publish_event(&client, &policy, &sleeper, args.type_id, &parsed_data, meta).await
}
