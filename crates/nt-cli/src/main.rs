mod auth;
mod commands;
mod credentials;
mod env;
mod paths;
mod transport;
mod urls;

use clap::{Parser, Subcommand};

use crate::env::SystemEnv;

#[derive(Parser)]
#[command(
    name = "nt",
    version,
    about = "no-tickets — ticketless project management for AI teams"
)]
struct Cli {
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
    let env = SystemEnv;
    let exit = match cli.command {
        Commands::Status => commands::status::run(&env),
        Commands::Publish {
            r#type,
            data,
            project,
        } => {
            commands::publish::run(
                commands::publish::PublishArgs {
                    type_id: &r#type,
                    data: &data,
                    project: &project,
                },
                &env,
            )
            .await
        }
    };
    std::process::exit(exit);
}
