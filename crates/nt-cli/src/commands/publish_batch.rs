//! `nt publish --file <path>` — batch publish from JSONL.
//!
//! Mirrors `src/cli/commands/publish/batch.ts::runPublishBatch` from the
//! TS reference. Reads JSONL (one JSON object per line) from a file path
//! (or stdin when path is `-`), validates each line locally, builds
//! one envelope per line with a per-line source override on top of the
//! CLI base source, and sends the lot as a single POST to `/v1/events`.
//!
//! Distinct from Task 4b (`--stream` mode): batch is one finite read
//! → one HTTP call → exit. Stream is a long-lived subprocess with
//! JSONL on stdin AND stdout.

mod envelope;
mod jsonl;
mod source;

use serde_json::Value;

use crate::auth::{emit_host_mismatch_warning, resolve_auth, AuthOutcome, NOT_AUTH_MSG};
use crate::env::Env;
use crate::source_detect::machine_hash_attribute;
use crate::transport::{
    post_json_with_retry, Client, HttpClient, RetryPolicy, Sleeper, TokioSleeper,
};
use crate::urls::resolve_urls;

use envelope::validate_and_build_envelope;
use jsonl::{parse_jsonl, read_batch_input};
use source::build_cli_base_source;

pub struct PublishBatchArgs<'a> {
    /// Path to a `.jsonl` file, or `-` to read from stdin.
    pub batch_path: &'a str,
    /// Project name; appears in `source.attributes.project` on every
    /// envelope in the batch (matches single-event behaviour).
    pub project: &'a str,
    /// Override the default `source.name` ("no-tickets-cli") on the CLI base
    /// source. JSONL lines may override per-line via their own
    /// `source.name`.
    pub source_name: Option<&'a str>,
    /// `--source-attribute KEY=VALUE` entries to seed
    /// `source.attributes` on every envelope. JSONL line attributes
    /// merge on top (line wins on key collisions).
    pub source_attributes: &'a [String],
}

/// Entry point. Reads input, parses JSONL, validates per line, merges
/// sources, sends the batch, prints the response, returns an exit code.
pub async fn run(args: PublishBatchArgs<'_>, env: &dyn Env) -> i32 {
    // 1. Read raw input from file or stdin.
    let raw = match read_batch_input(args.batch_path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 2. Parse JSONL — line-numbered errors point to the source file.
    let entries = match parse_jsonl(&raw) {
        Ok(es) => es,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 3. Empty batch is a usage error, not a no-op success.
    if entries.is_empty() {
        eprintln!("batch file \"{}\" is empty", args.batch_path);
        return 1;
    }

    // 4. Compute the CLI base source once for the whole batch. Machine
    //    hash is resolved here (same as single-event) so the entire
    //    batch attributes the same producing machine. Per-line source
    //    overrides merge on top of this base.
    let machine_hash_owned: Option<String> = machine_hash_attribute(env);
    let cli_source = match build_cli_base_source(
        args.source_name,
        args.project,
        args.source_attributes,
        machine_hash_owned.as_deref(),
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 5. Per-line validation + envelope construction. Any failure
    //    short-circuits with a line-numbered diagnostic and exit 1.
    let mut envelopes: Vec<Value> = Vec::with_capacity(entries.len());
    for entry in entries {
        match validate_and_build_envelope(&entry, &cli_source) {
            Ok(envelope) => envelopes.push(envelope),
            Err(msg) => {
                eprintln!("{msg}");
                return 1;
            }
        }
    }

    // 6. Resolve URLs + auth (same shape as single-event run()).
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
    let client = match Client::new(urls.api_url, auth.token) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    // 7. Single POST with the batch array; reuses retry/backoff from
    //    Task 17. Body serialises a `Vec<Value>` so each line's full
    //    envelope (incl. merged source) lands on the wire verbatim.
    let body_bytes = serde_json::to_vec(&envelopes).expect("envelope vec always serialises");
    let policy = RetryPolicy::default_publish();
    let sleeper = TokioSleeper;
    publish_envelopes(&client, &policy, &sleeper, &body_bytes).await
}

/// Post the serialised batch envelope array, print server response,
/// map to an exit code. Mirrors `commands::publish::publish_event` but
/// for a multi-envelope body.
async fn publish_envelopes<C: HttpClient, S: Sleeper>(
    client: &C,
    policy: &RetryPolicy,
    sleeper: &S,
    body_bytes: &[u8],
) -> i32 {
    match post_json_with_retry(client, policy, sleeper, "/v1/events", body_bytes).await {
        Ok(response) => {
            println!(
                "{}",
                serde_json::to_string(&response).expect("serde_json::Value always serialises"),
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
