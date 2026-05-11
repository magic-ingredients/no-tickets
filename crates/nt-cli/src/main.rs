mod auth;
mod commands;
mod credentials;
mod home;
mod urls;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "no-tickets — ticketless project management for AI teams"
)]
struct Cli {
    /// Profile name from ~/.notickets/config.json.
    /// Alternative to NO_TICKETS_API_URL / NO_TICKETS_AUTH_URL env vars.
    /// Global: works both before and after the subcommand.
    #[arg(long, global = true)]
    profile: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Print authentication and URL resolution status as JSON.
    Status,
    /// Publish a single event to the configured no-tickets API.
    /// Spike scope — single event only; no --stream, no local schema
    /// validation, no batching. (See fix doc Task 14.)
    Publish {
        /// Event type id (e.g., `ai.task.completed.v1`).
        #[arg(long)]
        r#type: String,
        /// Event payload as a JSON string.
        #[arg(long)]
        data: String,
        /// Project name; sent as `--project` for routing alongside the
        /// Bearer token.
        #[arg(long)]
        project: String,
    },
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();
    let exit = match cli.command {
        Commands::Status => commands::status::run(cli.profile.as_deref()),
        Commands::Publish {
            r#type,
            data,
            project,
        } => {
            let parsed_data: serde_json::Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("--data must be valid JSON: {e}");
                    std::process::exit(1);
                }
            };
            commands::publish::run(commands::publish::PublishArgs {
                type_id: &r#type,
                data: &parsed_data,
                project: &project,
                profile: cli.profile.as_deref(),
            })
            .await
        }
    };
    std::process::exit(exit);
}
