pub mod config;
pub mod error_map;
pub mod example_synth;
pub mod registry_cache;
pub mod server;
pub mod tools;

use rmcp::{transport::stdio, ServiceExt};
use server::NtServer;
use tracing_subscriber::EnvFilter;

/// Run the nt-mcp stdio server to completion.
///
/// Routes ALL logging to stderr — anything to stdout corrupts the
/// MCP JSON-RPC stream and causes Claude Code to silently disconnect.
/// The stdout-purity integration test pins this.
pub async fn run() -> anyhow::Result<()> {
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
