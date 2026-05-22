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
