//! `nt publish` — single-event publish over HTTPS with Bearer auth.
//! Mirrors `src/cli/commands/publish` + `src/transport/events.ts::publish`.
//!
//! Spike scope (Task 14): single event only. No batching, no `--stream`
//! mode, no local schema validation, no source merging beyond
//! auto-fill, no retries. Task 5 (full CLI port) owns the rest.

use serde::Serialize;
use serde_json::Value;

use crate::auth::{NOT_AUTH_MSG, resolve_auth};
use crate::transport::Client;
use crate::urls::resolve_urls;

pub struct PublishArgs<'a> {
    pub type_id: &'a str,
    pub data: &'a Value,
    pub project: &'a str,
    pub profile: Option<&'a str>,
}

/// Serialised event envelope. Field declaration order is preserved by
/// serde derive — `type` first, then `data`, then `source` — to match
/// the TS `eventSchema` emission order. The wire-body field-order test
/// pins this.
#[derive(Serialize)]
struct EventEnvelope<'a> {
    #[serde(rename = "type")]
    type_id: &'a str,
    data: &'a Value,
    source: Source<'a>,
}

#[derive(Serialize)]
struct Source<'a> {
    name: &'a str,
    #[serde(rename = "sdkVersion")]
    sdk_version: &'a str,
    /// Project name flows through `source.attributes.project` since the
    /// TS sourceSchema's `attributes: Record<string, string|number|bool>`
    /// is the documented escape hatch for caller context. The server's
    /// auth layer derives project context from the token; this field
    /// is informational for the spike.
    #[serde(skip_serializing_if = "Option::is_none")]
    attributes: Option<SourceAttributes<'a>>,
}

#[derive(Serialize)]
struct SourceAttributes<'a> {
    project: &'a str,
}

pub async fn run(args: PublishArgs<'_>) -> i32 {
    // URL resolution first (matches handleStatus pattern). A profile
    // error or partial-pair env-var setup wins over auth missing.
    let urls = match resolve_urls(args.profile) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    let Some(auth) = resolve_auth() else {
        eprintln!("{NOT_AUTH_MSG}");
        return 1;
    };

    let client = match Client::new(urls.api_url, auth.token) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };

    let envelope = EventEnvelope {
        type_id: args.type_id,
        data: args.data,
        source: Source {
            name: "nt-cli",
            sdk_version: env!("CARGO_PKG_VERSION"),
            attributes: Some(SourceAttributes {
                project: args.project,
            }),
        },
    };
    let body = vec![envelope];

    match client.post_json("/v1/events", &body).await {
        Ok(response) => {
            // Print verbatim. Server response shape:
            // `{ ingested, deduped, ids }`.
            println!(
                "{}",
                serde_json::to_string(&response)
                    .unwrap_or_else(|_| response.to_string())
            );
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
