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

/// `no-tickets update --help` must succeed after the rename and
/// surface a structural marker (`Usage: no-tickets update`) that
/// proves the subcommand is REGISTERED, not just mentioned in
/// docstring prose. Pre-rename this fails with clap's usage error
/// because `update` isn't a registered subcommand.
#[tokio::test]
async fn update_subcommand_help_succeeds() {
    let home = tempdir();
    let out = run_nt(home.path(), &[], &["update", "--help"]).await;
    assert_eq!(
        out.code, 0,
        "`no-tickets update --help` must succeed after rename; got stderr={:?}",
        out.stderr
    );
    // Structural marker — clap's `Usage:` line names the actual
    // resolved subcommand chain, not the docstring. Catches a
    // mis-named or unregistered subcommand that would otherwise
    // slip past a `stdout.contains("update")` check (which would
    // match the docstring "Update the no-tickets binary…" line).
    assert!(
        out.stdout.contains("Usage: no-tickets update"),
        "help output must include the `Usage: no-tickets update` line, \
         proving the subcommand is registered; got stdout={:?}",
        out.stdout
    );
}

/// The dropped `self-update` name must now error specifically as a
/// clap usage error (exit 2 with the "unrecognized subcommand"
/// phrase), not as a generic non-zero exit. The discrimination
/// matters: a future deprecation-shim that exits 1 with a custom
/// message would silently slip past a `code != 0` check, but the
/// no-backcompat-alias contract requires clap's standard rejection.
#[tokio::test]
async fn self_update_old_name_no_longer_resolves() {
    let home = tempdir();
    let out = run_nt(home.path(), &[], &["self-update", "--help"]).await;
    // Clap's exit code for unrecognized subcommand is 2 across all
    // 4.x versions. Pin it specifically so a custom shim with a
    // different exit code surfaces as a test failure (rather than
    // silently meeting a `code != 0` weak check).
    assert_eq!(
        out.code, 2,
        "`no-tickets self-update` must return clap's usage-error exit 2 \
         (not a deprecation shim with a different code); \
         stdout={:?} stderr={:?}",
        out.stdout, out.stderr,
    );
    // Pin the "unrecognized subcommand" phrase (clap's standard
    // wording across 4.x). Previously this asserted
    // `contains("self-update") || contains("update")` — tautological
    // because `"self-update"` contains `"update"`, and clap echoes
    // the offending token verbatim. The new pin discriminates
    // between "clap rejected the unknown name" and "some other
    // exit-2 caller emitted unrelated output".
    assert!(
        out.stderr.contains("unrecognized subcommand"),
        "stderr must carry clap's standard `unrecognized subcommand` \
         phrase confirming the rejection path; got: {:?}",
        out.stderr,
    );
}

/// Exercise the dispatch arm to catch a `--help`-only verification
/// blind spot: clap intercepts `--help` before any `Commands::Update`
/// match arm runs, so the two help tests above wouldn't catch a
/// `Commands::Update => commands::publish::run()` miswire. This
/// test invokes `update` with no extra args, then asserts the
/// output (whichever path the command happens to take — already-
/// up-to-date, fetch failure, version-skew refusal, etc.) didn't
/// come from clap and didn't come from a sister command's `run()`.
#[tokio::test]
async fn update_dispatch_arm_invokes_update_command_not_some_other() {
    let home = tempdir();
    let out = run_nt(home.path(), &[], &["update"]).await;
    let combined = format!("{}{}", out.stdout, out.stderr);

    // Negative pin: clap's usage error would mean dispatch never
    // reached our run(). Catches the "miswired or unregistered"
    // failure mode.
    assert!(
        !combined.contains("unrecognized subcommand"),
        "dispatch must reach commands::update::run(); clap rejected \
         the subcommand instead. stdout={:?} stderr={:?}",
        out.stdout,
        out.stderr,
    );

    // Positive pin: the output must match one of update's own
    // documented stdout/stderr signatures. Listed exhaustively so
    // a future code path that emits something else triggers a
    // test failure and forces a deliberate update here.
    let signatures = [
        "is already the latest version", // up-to-date path (stdout)
        "no-tickets release status:",    // self_update crate's progress (stdout)
        "Checking target-arch",          // self_update crate's progress (stderr)
        "Checking current version",      // self_update crate's progress (stderr)
        "no-tickets update:",            // our own eprintln-prefixed errors
    ];
    let routed = signatures.iter().any(|sig| combined.contains(sig));
    assert!(
        routed,
        "output didn't match any known commands::update::run() signature, \
         which means dispatch likely went somewhere else (publish? validate?). \
         stdout={:?} stderr={:?}",
        out.stdout, out.stderr,
    );
}
