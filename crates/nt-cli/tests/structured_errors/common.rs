//! Shared harness for structured-error contract tests.
//!
//! Spawns `nt <subcommand> <args>` with stderr piped (so the binary's
//! TTY detection sees a non-terminal and emits JSON), with hermetic
//! env: an isolated `NO_TICKETS_HOME` per test, no host
//! `NO_TICKETS_TOKEN` leaking in unless the caller opts in, and a
//! wiremock URL for the publish suite.
//!
//! Returns the captured exit + stdout + stderr so test assertions can
//! both check the exit-code contract and parse stderr as JSON.

use std::path::Path;
use std::process::Stdio;

use assert_cmd::cargo::cargo_bin;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

#[derive(Debug)]
pub(crate) struct Output {
    pub(crate) code: i32,
    #[allow(dead_code)]
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl Output {
    /// Parse stderr as a single-line JSON object. Trims the trailing
    /// newline the emitter writes. Panics with a helpful message if
    /// stderr isn't a valid JSON line, since that means the binary
    /// failed the contract (the whole point of these tests).
    pub(crate) fn stderr_json(&self) -> serde_json::Value {
        let line = self.stderr.trim_end();
        serde_json::from_str(line).unwrap_or_else(|e| {
            panic!(
                "stderr is not a single-line JSON object \
                 (binary failed the structured-error contract): {e}\n\
                 stderr was: {:?}",
                self.stderr
            )
        })
    }
}

/// Run `nt <args...>` with hermetic env. `extra_env` overrides defaults.
pub(crate) async fn run_nt(home: &Path, extra_env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut cmd = Command::new(cargo_bin("no-tickets"));
    cmd.env("NO_TICKETS_HOME", home)
        // Prevent the host shell from leaking credentials/URLs into
        // the subprocess. Each test opts in by passing values via
        // `extra_env`.
        .env_remove("NO_TICKETS_TOKEN")
        .env_remove("NO_TICKETS_API_URL")
        .env_remove("NO_TICKETS_AUTH_URL")
        .env_remove("NO_TICKETS_ENV")
        .env_remove("NO_TICKETS_INCLUDE_MACHINE")
        // Collapse retry sleeps for the transport-error case.
        .env("NT_RETRY_BASE_DELAY_MS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    for a in args {
        cmd.arg(a);
    }
    let mut child = cmd.spawn().expect("spawn nt binary");
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut stdout = child.stdout.take().expect("stdout pipe");
    let mut stderr = child.stderr.take().expect("stderr pipe");
    let (s_out, s_err, status) = tokio::join!(
        stdout.read_to_end(&mut stdout_buf),
        stderr.read_to_end(&mut stderr_buf),
        child.wait(),
    );
    s_out.expect("read stdout");
    s_err.expect("read stderr");
    let status = status.expect("child exits");
    Output {
        code: status.code().unwrap_or(-1),
        stdout: String::from_utf8(stdout_buf).expect("stdout utf8"),
        stderr: String::from_utf8(stderr_buf).expect("stderr utf8"),
    }
}

pub(crate) fn tempdir() -> tempfile::TempDir {
    tempfile::tempdir().expect("tempdir")
}
