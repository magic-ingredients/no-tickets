mod config;
mod error_map;
mod example_synth;
mod registry_cache;
mod server;
mod tools;

use rmcp::{transport::stdio, ServiceExt};
use server::NtServer;
use tracing_subscriber::EnvFilter;

/// Run the nt-mcp stdio server to completion.
///
/// Routes ALL logging to stderr — anything to stdout corrupts the
/// MCP JSON-RPC stream and causes Claude Code to silently disconnect.
/// The stdout-purity integration test pins this.
pub async fn run() -> anyhow::Result<()> {
    // `try_init` (not `init`) so a re-entrant subscriber install — e.g.
    // an embedded-future test path that already installed one — is a
    // no-op rather than a panic.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .try_init();

    tracing::info!("nt-mcp starting (stdio transport)");

    let service = NtServer::new().serve(stdio()).await.inspect_err(|e| {
        tracing::error!("rmcp serve error: {e:?}");
    })?;
    service.waiting().await?;
    Ok(())
}
