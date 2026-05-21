mod atomic_write;
mod auth;
mod auth_server;
mod clock;
mod commands;
mod config;
mod credentials;
mod env;
mod error;
mod paths;
mod session;
mod source_detect;
mod state;
mod transport;
mod urls;

use std::io::IsTerminal;

use clap::{Parser, Subcommand};

use crate::clock::SystemClock;
use crate::env::SystemEnv;
use crate::error::emit_and_exit_code;

#[derive(Parser)]
#[command(
    name = "no-tickets",
    version,
    about = "no-tickets — ticketless project management for AI teams"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authenticate via the browser and save session credentials.
    Init,
    /// Delete local session credentials. Symmetric with `init`.
    Logout,
    /// Print authentication and locally-registered push tokens as JSON.
    Status,
    /// Publish one or more events to the configured no-tickets API.
    ///
    /// Two modes:
    ///
    /// {n}- Single-event: `--type` + `--data` (a JSON payload string).
    ///
    /// {n}- Batch: `--file <path>` (or `-` for stdin) — JSONL, one
    /// event object per line. Each line may carry its own `source`
    /// override; otherwise the CLI base source is applied.
    ///
    /// `--file` is mutually exclusive with `--type` and `--data`.
    Publish {
        /// Event type id (e.g., `ai.task.completed.v1`). Required in
        /// single-event mode; unused (and forbidden) with `--file`.
        #[arg(long, conflicts_with = "file", required_unless_present = "file")]
        r#type: Option<String>,
        /// Event payload as a JSON string. Required in single-event
        /// mode; unused (and forbidden) with `--file`.
        #[arg(long, conflicts_with = "file", required_unless_present = "file")]
        data: Option<String>,
        /// Read a JSONL batch of events from <PATH>, or `-` for stdin.
        /// One JSON object per line. Mutually exclusive with `--type`
        /// and `--data`.
        #[arg(long, value_name = "PATH")]
        file: Option<String>,
        /// Local project key — looks up the push token registered via
        /// `no-tickets token add`. The server resolves the actual project
        /// from the token; this value is not sent on the wire.
        #[arg(long)]
        project: String,
        /// Override the default `source.name` ("no-tickets-cli").
        #[arg(long)]
        source_name: Option<String>,
        /// Add an attribute to `source.attributes` as `KEY=VALUE`. May be
        /// repeated; last value wins on duplicate keys.
        #[arg(long = "source-attribute", value_name = "KEY=VALUE")]
        source_attribute: Vec<String>,
        /// Parent event id (`parentEventId` on the wire).
        #[arg(long)]
        parent: Option<String>,
        /// Trace id (`traceId` on the wire).
        #[arg(long)]
        trace: Option<String>,
        /// Idempotency key (`dedupeKey` on the wire).
        #[arg(long)]
        dedupe_key: Option<String>,
    },
    /// Manage locally-registered push tokens (paste from the web UI).
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
    /// Validate a payload locally against the bundled JSON Schema for
    /// the given event type. No auth, no network.
    Validate {
        /// Event type id (e.g., `ai.task.completed.v1`).
        #[arg(long)]
        r#type: String,
        /// Event payload as a JSON string.
        #[arg(long)]
        data: String,
    },
    /// Update the no-tickets binary (install.sh / direct-download installs only).
    Update,
    /// Declare an agent-harness identity for opt-in actor attribution.
    ///
    /// `start` writes <config-dir>/active-session.json so subsequent
    /// `no-tickets publish` invocations stamp `metadata.actor` automatically.
    /// `show` prints the active session as JSON. `end` deletes the session
    /// file and clears the first-publish hint marker.
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Declare an agent identity. Only `--agent` is required.
    Start {
        /// Agent id (e.g., `claude`, `codex`, `tiny-brain`, `github-actions`).
        #[arg(long)]
        agent: String,
        /// LLM model name (omit for non-LLM systems).
        #[arg(long)]
        model: Option<String>,
        /// LLM provider (e.g., `anthropic`, `openai`).
        #[arg(long)]
        provider: Option<String>,
        /// Thinking-effort hint: `low`, `medium`, or `high`.
        #[arg(long = "thinking-effort", value_parser = ["low", "medium", "high"])]
        thinking_effort: Option<String>,
        /// Opaque session id for grouping events from one harness run.
        #[arg(long = "session-id")]
        session_id: Option<String>,
        /// Max age of the session before `show`/publish treat it as
        /// expired. Default 24, valid range 1..=168 (7-day hard cap).
        #[arg(
            long = "max-age-hours",
            default_value_t = 24,
            value_parser = clap::value_parser!(u32).range(1..=168)
        )]
        max_age_hours: u32,
    },
    /// Print the active session as JSON, or `{"active":false}`.
    Show,
    /// Delete the active-session file and clear the hint marker.
    /// Idempotent — succeeds when no session is set.
    End,
}

#[derive(Subcommand)]
enum TokenAction {
    /// Register a push token for a project. The token is stored locally;
    /// no server call is made.
    Add {
        /// Project name (key in the local registry).
        project: String,
        /// Push token (must begin with `nt_push_`).
        push_token: String,
        /// Free-text label, surfaced by `nt status` / `nt token list`.
        #[arg(long)]
        label: Option<String>,
        /// Overwrite an existing entry for the project.
        #[arg(long)]
        force: bool,
    },
    /// List locally-registered tokens (project, masked token, addedAt, label).
    List,
    /// Drop a project entry from the local registry. Does NOT revoke
    /// server-side — use the web UI for that.
    Remove {
        /// Project name to remove.
        project: String,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let env = SystemEnv;
    let exit = match cli.command {
        Commands::Init => commands::init::run(&env),
        Commands::Logout => commands::logout::run(&env),
        Commands::Status => commands::status::run(&env),
        Commands::Publish {
            r#type,
            data,
            file,
            project,
            source_name,
            source_attribute,
            parent,
            trace,
            dedupe_key,
        } => {
            let is_tty = std::io::stderr().is_terminal();
            if let Some(batch_path) = file.as_deref() {
                // Per-event metadata flags (--parent, --trace,
                // --dedupe-key) are single-event-only: each batch line
                // carries its own envelope-level metadata. We could
                // clap-conflict these too, but the cost of a surface
                // that quietly ignores them is high (silent data loss);
                // reject early with a clear message.
                if parent.is_some() || trace.is_some() || dedupe_key.is_some() {
                    let err = error::NtError::Usage {
                        message: "--file is incompatible with --parent/--trace/--dedupe-key. \
                                  Per-event metadata in batch mode lives in each JSONL line."
                            .to_string(),
                    };
                    emit_and_exit_code(Err(err), &mut std::io::stderr().lock(), is_tty)
                } else {
                    // Batch flow still emits errors via eprintln; full
                    // migration to NtError tracked as a follow-up
                    // cleanup ticket per Task 26's scope.
                    commands::publish_batch::run(
                        commands::publish_batch::PublishBatchArgs {
                            batch_path,
                            project: &project,
                            source_name: source_name.as_deref(),
                            source_attributes: &source_attribute,
                        },
                        &env,
                    )
                    .await
                }
            } else {
                // clap's `required_unless_present = "file"` guarantees
                // both --type and --data are Some when --file is None.
                let r#type = r#type.expect("clap required_unless_present");
                let data = data.expect("clap required_unless_present");
                let result = commands::publish::run(
                    commands::publish::PublishArgs {
                        type_id: &r#type,
                        data: &data,
                        project: &project,
                        source_name: source_name.as_deref(),
                        source_attributes: &source_attribute,
                        parent: parent.as_deref(),
                        trace: trace.as_deref(),
                        dedupe_key: dedupe_key.as_deref(),
                    },
                    &env,
                )
                .await;
                emit_and_exit_code(result, &mut std::io::stderr().lock(), is_tty)
            }
        }
        Commands::Token { action } => match action {
            TokenAction::Add {
                project,
                push_token,
                label,
                force,
            } => commands::token_add::run(&env, &project, &push_token, label.as_deref(), force),
            TokenAction::List => commands::token_list::run(&env),
            TokenAction::Remove { project } => commands::token_remove::run(&env, &project),
        },
        Commands::Validate { r#type, data } => {
            let is_tty = std::io::stderr().is_terminal();
            let result = commands::validate::run(&r#type, &data);
            emit_and_exit_code(result, &mut std::io::stderr().lock(), is_tty)
        }
        Commands::Update => commands::update::run().await,
        Commands::Session { action } => {
            let clock = SystemClock;
            match action {
                SessionAction::Start {
                    agent,
                    model,
                    provider,
                    thinking_effort,
                    session_id,
                    max_age_hours,
                } => commands::session::run_start(
                    &env,
                    &clock,
                    commands::session::StartArgs {
                        agent: &agent,
                        model: model.as_deref(),
                        provider: provider.as_deref(),
                        thinking_effort: thinking_effort.as_deref(),
                        session_id: session_id.as_deref(),
                        max_age_hours,
                    },
                ),
                SessionAction::Show => commands::session::run_show(&env, &clock),
                SessionAction::End => commands::session::run_end(&env),
            }
        }
    };
    std::process::exit(exit);
}
