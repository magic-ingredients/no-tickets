mod auth;
mod auth_server;
mod commands;
mod config;
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
    /// Authenticate via the browser and save session credentials.
    Init,
    /// Delete local session credentials. Symmetric with `init`.
    Logout,
    /// Print authentication and locally-registered push tokens as JSON.
    Status,
    /// Publish a single event to the configured no-tickets API.
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
    /// Manage locally-registered push tokens (paste from the web UI).
    Token {
        #[command(subcommand)]
        action: TokenAction,
    },
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
    };
    std::process::exit(exit);
}
