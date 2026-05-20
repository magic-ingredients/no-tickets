// `anyhow::Result` is the idiomatic binary-entry error type — `main`
// doesn't need typed errors, only "exit with a backtrace if something
// went wrong."
//
// `--help` / `-h` / `--version` / `-V` are intercepted before
// `nt_mcp::run()` so a user who reflexively types `no-tickets-mcp --help`
// gets an explanatory message instead of stdio-MCP errors when no
// client is connected.

use std::env;

const HELP: &str = "\
no-tickets-mcp — MCP server for no-tickets

Speaks JSON-RPC over stdio. Intended for invocation by MCP clients
(Claude Code, Cursor, etc.) — not for direct human use.

USAGE:
    no-tickets-mcp                Start the server on stdio.

CONFIGURATION (env vars):
    NO_TICKETS_TOKEN              Push token. Required for publish_event.
    NO_TICKETS_API_URL            Override the no-tickets API URL.

MCP CLIENT CONFIG (example):
    {
      \"command\": \"no-tickets-mcp\",
      \"args\": []
    }

See https://github.com/magic-ingredients/no-tickets for full docs.
";

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("no-tickets-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => {}
        }
    }
    nt_mcp::run().await
}
