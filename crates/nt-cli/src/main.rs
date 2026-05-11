mod auth;
mod credentials;
mod home;
mod status;
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
}

fn main() {
    let cli = Cli::parse();
    let exit = match cli.command {
        Commands::Status => status::run(cli.profile.as_deref()),
    };
    std::process::exit(exit);
}
