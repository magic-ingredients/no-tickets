//! Integration tests for `no-tickets internal generate-docs <target>`.
//!
//! Pins the end-to-end behaviour the docs-site sync workflow relies on:
//! every concrete subcommand from the real `Cli` struct lands as a
//! Mintlify-compatible MDX file in the target directory; hidden
//! commands (including the `internal` group itself) don't appear; the
//! emit is idempotent so an unchanged run produces a clean diff.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn nt() -> Command {
    Command::cargo_bin("no-tickets").expect("binary built")
}

fn target_with_docs() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    let path = temp.path().to_path_buf();
    nt().args([
        "internal",
        "generate-docs",
        path.to_str().expect("utf8 path"),
    ])
    .assert()
    .success();
    temp
}

fn read(target: &Path, relative: &str) -> String {
    fs::read_to_string(target.join(relative))
        .unwrap_or_else(|e| panic!("read {relative} under {target:?}: {e}"))
}

/// Slice an MDX body between `## <section>` and the next `## ` heading
/// (or end-of-file). Returns `None` if the section header isn't
/// present. Used by integration tests to anchor "this content appears
/// in section X" assertions instead of relying on body-wide substring
/// matches.
fn section<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let header = format!("## {name}\n");
    let start = body.find(&header)?;
    let after_header = &body[start + header.len()..];
    let end = after_header
        .find("\n## ")
        .map(|n| start + header.len() + n)
        .unwrap_or(body.len());
    Some(&body[start..end])
}

// ─── subcommand existence ──────────────────────────────────────────────────

#[test]
fn generate_docs_emits_publish_mdx() {
    let target = target_with_docs();
    assert!(
        target.path().join("publish.mdx").exists(),
        "publish.mdx must land under <target>",
    );
}

#[test]
fn generate_docs_emits_session_subcommand_pages() {
    // The `session` command group landed in event-actor-metadata
    // Phase 1. Pin that the group + each child gets a page so the
    // docs site keeps up automatically.
    let target = target_with_docs();
    assert!(
        target.path().join("session.mdx").exists(),
        "session.mdx (group) must be emitted",
    );
    for child in &["start", "show", "end"] {
        let rel = format!("session/{child}.mdx");
        assert!(target.path().join(&rel).exists(), "{rel} must be emitted",);
    }
}

#[test]
fn generate_docs_emits_token_subcommand_pages() {
    let target = target_with_docs();
    assert!(target.path().join("token.mdx").exists());
    for child in &["add", "list", "remove"] {
        let rel = format!("token/{child}.mdx");
        assert!(target.path().join(&rel).exists(), "{rel} must be emitted",);
    }
}

// ─── hidden surface stays hidden ───────────────────────────────────────────

#[test]
fn generate_docs_does_not_emit_internal_group_or_its_children() {
    // The `internal` group is the one running this test. It must
    // not appear in the public docs surface. A regression that
    // forgets to mark `Internal` as `hide = true` would leak the
    // build-only `generate-docs` command into docs.no-tickets.com.
    let target = target_with_docs();
    assert!(
        !target.path().join("internal.mdx").exists(),
        "hidden `internal` group must not appear in MDX output",
    );
    assert!(
        !target.path().join("internal/generate-docs.mdx").exists(),
        "children of hidden groups must not appear",
    );
}

// ─── frontmatter + sections on a real command ──────────────────────────────

#[test]
fn generated_publish_mdx_has_frontmatter_and_required_sections() {
    let target = target_with_docs();
    let body = read(target.path(), "publish.mdx");

    assert!(
        body.starts_with("---\n"),
        "MDX must start with Mintlify frontmatter fence; got first 40 bytes: {:?}",
        &body[..body.len().min(40)],
    );
    assert!(
        body.contains("title: publish\n"),
        "frontmatter title must be `publish`; body:\n{body}",
    );
    assert!(
        body.contains("## Usage"),
        "publish.mdx must include `## Usage`; body:\n{body}",
    );
    assert!(
        body.contains("## Flags"),
        "publish has flags → `## Flags` must appear; body:\n{body}",
    );
    // The wire field on `publish` includes `--actor-type` (added in
    // event-actor-metadata Phase 1). Anchor the assertion to the
    // `## Flags` section so a regression that moves the table out
    // (or accidentally inlines the flag in narrative prose only)
    // still fails the test.
    let flags = section(&body, "Flags").expect("## Flags section present");
    assert!(
        flags.contains("--actor-type"),
        "## Flags section must include `--actor-type`; section was:\n{flags}",
    );
}

#[test]
fn generated_publish_mdx_does_not_emit_help_or_version_rows_in_flags_table() {
    // Clap auto-injects `--help`/`--version` flags on every command.
    // The MDX flag table must skip them — they're meta-flags every
    // CLI has, not part of the command's actual interface.
    let target = target_with_docs();
    let body = read(target.path(), "publish.mdx");
    let flags = section(&body, "Flags").expect("## Flags section present");
    assert!(
        !flags.contains("`--help`"),
        "auto-injected `--help` must NOT appear in the flag table; section was:\n{flags}",
    );
    assert!(
        !flags.contains("`--version`"),
        "auto-injected `--version` must NOT appear in the flag table; section was:\n{flags}",
    );
}

#[test]
fn generated_session_start_mdx_uses_full_invocation_path_as_title() {
    let target = target_with_docs();
    let body = read(target.path(), "session/start.mdx");
    assert!(
        body.contains("title: session start\n"),
        "nested-command title must be full invocation path `session start`; body:\n{body}",
    );
}

// ─── snapshot fixtures ─────────────────────────────────────────────────────
//
// The emitter's output is the wire contract between this repo and
// `no-tickets-docs`. Field-level assertions (above) pin individual
// invariants; the snapshot test pins the *exact* byte sequence so a
// clap-derive macro upgrade or a renderer tweak that changes the
// rendered shape — even cosmetically — fails loudly and forces a
// reviewed fixture update.
//
// Fixtures live at `crates/nt-cli/tests/snapshots/`. To regenerate
// after an intentional change:
//
//     cargo run --bin no-tickets -- internal generate-docs \
//         crates/nt-cli/tests/snapshots
//
// Then `git diff` shows the contract change for review.

const SNAPSHOT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/snapshots");

fn walk_mdx(root: &Path) -> Vec<std::path::PathBuf> {
    fn recurse(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}")) {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                recurse(&path, out);
            } else if path.extension().is_some_and(|e| e == "mdx") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    recurse(root, &mut out);
    out.sort();
    out
}

fn relative_paths(root: &Path) -> Vec<String> {
    walk_mdx(root)
        .into_iter()
        .map(|p| {
            p.strip_prefix(root)
                .expect("under root")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}

#[test]
fn snapshot_file_set_matches_committed_fixtures() {
    // Detects: new commands added without a fixture, or fixtures left
    // behind after a command was removed. Either case is a contract
    // change that needs an explicit fixture update.
    let target = target_with_docs();
    let snapshot_root = Path::new(SNAPSHOT_DIR);
    assert!(
        snapshot_root.is_dir(),
        "snapshot fixtures directory missing: {SNAPSHOT_DIR}\n\
         regenerate with: cargo run --bin no-tickets -- internal generate-docs {SNAPSHOT_DIR}",
    );
    let actual = relative_paths(target.path());
    let expected = relative_paths(snapshot_root);
    assert_eq!(
        actual, expected,
        "emitter file set drifted from committed fixtures\n\
         regenerate with: cargo run --bin no-tickets -- internal generate-docs {SNAPSHOT_DIR}",
    );
}

#[test]
fn snapshot_file_contents_match_committed_fixtures() {
    // Byte-identical match per file. A diff in any single file fails
    // the test and prints which file drifted so the dev can inspect
    // the change before regenerating fixtures.
    let target = target_with_docs();
    let snapshot_root = Path::new(SNAPSHOT_DIR);
    assert!(
        snapshot_root.is_dir(),
        "snapshot fixtures directory missing: {SNAPSHOT_DIR}\n\
         regenerate with: cargo run --bin no-tickets -- internal generate-docs {SNAPSHOT_DIR}",
    );
    for relative in relative_paths(snapshot_root) {
        let actual = read(target.path(), &relative);
        let expected = read(snapshot_root, &relative);
        assert_eq!(
            actual, expected,
            "snapshot drift in {relative}\n\
             regenerate with: cargo run --bin no-tickets -- internal generate-docs {SNAPSHOT_DIR}",
        );
    }
}

// ─── idempotence ──────────────────────────────────────────────────────────

#[test]
fn generate_docs_is_idempotent_across_runs() {
    // Two consecutive runs against the same target must produce
    // byte-identical files — the docs-site sync workflow's
    // diff-aware "no-op when unchanged" exit depends on this.
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().to_str().unwrap();
    nt().args(["internal", "generate-docs", path])
        .assert()
        .success();
    let first = read(temp.path(), "publish.mdx");
    nt().args(["internal", "generate-docs", path])
        .assert()
        .success();
    let second = read(temp.path(), "publish.mdx");
    assert_eq!(
        first, second,
        "publish.mdx must be byte-identical across two emitter runs",
    );
}
