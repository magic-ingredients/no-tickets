// `anyhow::Result` mirrors the original `nt-mcp/src/main.rs` and is the
// idiomatic binary-entry error type — `main` doesn't need typed errors,
// only "exit with a backtrace if something went wrong."

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    nt_mcp::run().await
}
