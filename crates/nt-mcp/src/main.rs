mod config;
mod example_synth;
mod fixtures;
mod server;
mod tools;

use rmcp::{transport::stdio, ServiceExt};
use server::NtServer;
use tracing_subscriber::EnvFilter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    // CRITICAL: route ALL logging to stderr. Anything to stdout corrupts
    // the MCP JSON-RPC stream and causes Claude Code to silently
    // disconnect. The stdout-purity integration test pins this.
    //
    // `try_init` so a re-entrant subscriber install (e.g., embedded
    // future usage in tests) is a no-op rather than a panic.
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
