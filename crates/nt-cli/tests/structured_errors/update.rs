//! `nt update` (formerly `nt self-update`) — subcommand-rename pins.
//!
//! Task 2 of `docs/fixes/self-update-broken-on-tar-xz.md` renames
//! the binary-update subcommand from `self-update` to `update`. The
//! `self-` prefix is a rustup carry-over that carried no information
//! for no-tickets (no managed sub-things to update) and actively
//! misled users into expecting auto-update behaviour. The new name
//! matches the muscle memory from `brew upgrade`, `apt update`, etc.
//!
//! No backcompat alias per `[[project_no_v1_backcompat]]` — the old
//! name now errors with clap's standard "unrecognized subcommand"
//! message. Subprocess-level (not type-level) so RED tests can
//! compile against the pre-rename binary and fail at runtime,
//! avoiding the bundled-RED+GREEN pattern that the publish-uses-
//! push-token Task 2 had to use for its NtError variant addition.

use crate::common::{run_nt, tempdir};

/// `no-tickets update --help` must succeed after the rename. Pre-
/// rename this fails because `update` isn't a registered subcommand
/// and clap returns a usage error.
#[tokio::test]
async fn update_subcommand_help_succeeds() {
    let home = tempdir();
    let out = run_nt(home.path(), &[], &["update", "--help"]).await;
    assert_eq!(
        out.code, 0,
        "`no-tickets update --help` must succeed after rename; got stderr={:?}",
        out.stderr
    );
    // Help text on stdout (clap convention) must name the new command.
    assert!(
        out.stdout.to_lowercase().contains("update"),
        "help text must surface `update`; got: {}",
        out.stdout
    );
}

/// The dropped `self-update` name must now error. Clap's default
/// behaviour for an unrecognized subcommand exits non-zero and
/// names the offending token on stderr; some clap versions also
/// suggest the nearest match. We don't pin the exact exit code or
/// message text (clap is upstream) — only that the command no
/// longer succeeds.
#[tokio::test]
async fn self_update_old_name_no_longer_resolves() {
    let home = tempdir();
    let out = run_nt(home.path(), &[], &["self-update", "--help"]).await;
    assert_ne!(
        out.code, 0,
        "`no-tickets self-update` must NOT succeed post-rename (dropped name); \
         stdout={:?} stderr={:?}",
        out.stdout, out.stderr,
    );
    // Stderr should mention the offending name OR offer a `update`
    // suggestion. Either signal is enough — pinning both would tie
    // the test to clap's exact wording across versions.
    let combined = format!("{}{}", out.stdout, out.stderr).to_lowercase();
    assert!(
        combined.contains("self-update") || combined.contains("update"),
        "expected clap to mention the unrecognized subcommand or its suggestion; \
         stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr,
    );
}
