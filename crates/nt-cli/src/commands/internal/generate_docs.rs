//! `no-tickets internal generate-docs <target>` — walk the Clap tree
//! and emit one Mintlify-flavoured MDX page per subcommand into
//! `<target>/`. The docs-site repo consumes these pages from a
//! release-tag workflow that PRs the diff into `no-tickets-docs`.
//!
//! The output is the wire contract between this binary and the docs
//! site. Snapshot tests pin the rendered shape so a clap-derive
//! macro upgrade can't silently change the table format.
//!
//! Three properties pinned by tests:
//!
//! 1. **Complete** — every concrete subcommand and command group
//!    gets a page (groups so users discover their children; concrete
//!    commands for the actual invocation reference).
//! 2. **Hidden commands stay hidden** — anything marked `hide = true`
//!    (including this `internal` group itself) is omitted.
//! 3. **Idempotent** — re-emitting against the same target produces
//!    byte-identical output. No timestamps in frontmatter, no
//!    nondeterministic argument-iteration order.

use std::path::{Path, PathBuf};

/// One emitted MDX page. `path` is relative to the caller's target
/// directory and uses `/` separators for nested commands; the CLI
/// handler joins it with the absolute target root before writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedFile {
    pub path: PathBuf,
    pub content: String,
}

/// Walk `root`'s subcommand tree and produce one [`EmittedFile`] per
/// non-hidden command. The root itself is skipped — `no-tickets` as
/// a whole has its own hand-written overview page in the docs-site
/// repo; this emitter handles the per-command reference only.
#[allow(dead_code, unused_variables)] // wired in by GREEN
pub fn emit_docs(root: &clap::Command) -> Vec<EmittedFile> {
    // RED stub: returns nothing. Tests that expect at least one
    // file fail; the "no subcommands → empty output" test passes
    // (correctly, by accident).
    Vec::new()
}

/// CLI entrypoint for `no-tickets internal generate-docs <target>`.
/// Resolves the Clap tree via `<Cli as CommandFactory>::command()` in
/// `main.rs` (so the emitter sees the exact same surface as live
/// argument parsing), calls [`emit_docs`], and writes each file under
/// `target`.
#[allow(dead_code, unused_variables)] // wired in by GREEN
pub fn run(target: &Path) -> i32 {
    // RED stub: pretends success without writing anything.
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, Command};

    // ─── helpers ───────────────────────────────────────────────────────────

    /// Synthetic Clap tree mirroring the real `no-tickets` surface
    /// shape (a couple of top-level commands, one command group with
    /// nested children, one hidden group) without dragging the real
    /// `Cli` struct into unit-test land.
    fn sample_root() -> Command {
        Command::new("no-tickets")
            .subcommand(
                Command::new("publish")
                    .about("Publish events.")
                    .arg(Arg::new("type").long("type").required(true))
                    .arg(Arg::new("data").long("data").required(true)),
            )
            .subcommand(Command::new("status").about("Print auth + token state."))
            .subcommand(
                Command::new("token")
                    .about("Manage push tokens.")
                    .subcommand(
                        Command::new("add")
                            .about("Add a token.")
                            .arg(Arg::new("project").required(true)),
                    )
                    .subcommand(Command::new("list").about("List tokens.")),
            )
            .subcommand(
                Command::new("internal")
                    .about("Build-only tooling.")
                    .hide(true)
                    .subcommand(Command::new("generate-docs").about("Emit MDX.")),
            )
    }

    fn find_file<'a>(files: &'a [EmittedFile], path: &str) -> Option<&'a EmittedFile> {
        files.iter().find(|f| f.path == Path::new(path))
    }

    // ─── tree shape ────────────────────────────────────────────────────────

    #[test]
    fn emit_docs_returns_empty_for_command_with_no_subcommands() {
        let cmd = Command::new("nt").about("Bare binary.");
        assert!(
            emit_docs(&cmd).is_empty(),
            "no subcommands → no MDX pages emitted",
        );
    }

    #[test]
    fn emit_docs_emits_one_file_per_concrete_top_level_subcommand() {
        let files = emit_docs(&sample_root());
        assert!(
            find_file(&files, "publish.mdx").is_some(),
            "publish.mdx must be emitted; got {:?}",
            files.iter().map(|f| &f.path).collect::<Vec<_>>(),
        );
        assert!(
            find_file(&files, "status.mdx").is_some(),
            "status.mdx must be emitted; got {:?}",
            files.iter().map(|f| &f.path).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn emit_docs_emits_a_page_for_command_groups() {
        // Command groups (parents with subcommands of their own)
        // get their own page so users can discover the children.
        // `token` is a group with `add` + `list` underneath.
        let files = emit_docs(&sample_root());
        assert!(
            find_file(&files, "token.mdx").is_some(),
            "token.mdx (group page) must be emitted; got {:?}",
            files.iter().map(|f| &f.path).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn emit_docs_nests_subcommand_paths_under_group_directory() {
        let files = emit_docs(&sample_root());
        assert!(
            find_file(&files, "token/add.mdx").is_some(),
            "token/add.mdx must be emitted with `/`-separated path; got {:?}",
            files.iter().map(|f| &f.path).collect::<Vec<_>>(),
        );
        assert!(
            find_file(&files, "token/list.mdx").is_some(),
            "token/list.mdx must be emitted; got {:?}",
            files.iter().map(|f| &f.path).collect::<Vec<_>>(),
        );
    }

    #[test]
    fn emit_docs_uses_mdx_extension_for_every_file() {
        let files = emit_docs(&sample_root());
        assert!(
            !files.is_empty(),
            "precondition: emitter must produce some files for sample_root",
        );
        for f in &files {
            assert_eq!(
                f.path.extension().and_then(|e| e.to_str()),
                Some("mdx"),
                "every emitted file must use .mdx extension; got {:?}",
                f.path,
            );
        }
    }

    #[test]
    fn emit_docs_skips_hidden_commands_and_their_children() {
        // The `internal` group above is marked `hide(true)`. Neither
        // `internal.mdx` nor `internal/generate-docs.mdx` should appear
        // in output — otherwise the public docs site leaks the build
        // tooling.
        let files = emit_docs(&sample_root());
        assert!(
            find_file(&files, "internal.mdx").is_none(),
            "hidden group must not produce its own page",
        );
        assert!(
            find_file(&files, "internal/generate-docs.mdx").is_none(),
            "children of a hidden group must not be emitted",
        );
    }

    // ─── frontmatter ───────────────────────────────────────────────────────

    #[test]
    fn emit_docs_renders_frontmatter_title_as_full_command_path() {
        let files = emit_docs(&sample_root());
        let publish = find_file(&files, "publish.mdx").expect("publish.mdx emitted");
        assert!(
            publish.content.contains("title: publish\n"),
            "top-level title must be just `publish`; got:\n{}",
            publish.content,
        );

        let add = find_file(&files, "token/add.mdx").expect("token/add.mdx emitted");
        assert!(
            add.content.contains("title: token add\n"),
            "nested title must be the full invocation path `token add`; got:\n{}",
            add.content,
        );
    }

    #[test]
    fn emit_docs_renders_frontmatter_description_from_about_text() {
        let files = emit_docs(&sample_root());
        let publish = find_file(&files, "publish.mdx").expect("publish.mdx emitted");
        assert!(
            publish.content.contains("description: Publish events."),
            "description must come from the command's `about` text; got:\n{}",
            publish.content,
        );
    }

    #[test]
    fn emit_docs_emits_frontmatter_block_with_triple_dash_fences() {
        // Mintlify requires the standard YAML frontmatter fence shape
        // (`---\n…\n---\n`) at the very top of every MDX file.
        let files = emit_docs(&sample_root());
        for f in &files {
            assert!(
                f.content.starts_with("---\n"),
                "file {:?} must start with `---\\n`; got first 40 bytes: {:?}",
                f.path,
                &f.content[..f.content.len().min(40)],
            );
            // Second fence after the YAML body. Find the second `---`
            // line; everything before it is the frontmatter block.
            let after_first = &f.content[4..];
            assert!(
                after_first.contains("\n---\n"),
                "file {:?} must close its frontmatter with `\\n---\\n`; got:\n{}",
                f.path,
                f.content,
            );
        }
    }

    // ─── sections ──────────────────────────────────────────────────────────

    #[test]
    fn emit_docs_renders_usage_section() {
        let files = emit_docs(&sample_root());
        let publish = find_file(&files, "publish.mdx").expect("publish.mdx emitted");
        assert!(
            publish.content.contains("## Usage"),
            "publish.mdx must contain `## Usage` section; got:\n{}",
            publish.content,
        );
        // Synopsis line should mention the full invocation path —
        // an integrator copying the snippet should see
        // `no-tickets publish` rather than just `publish`.
        assert!(
            publish.content.contains("no-tickets publish"),
            "synopsis must include the full invocation; got:\n{}",
            publish.content,
        );
    }

    #[test]
    fn emit_docs_renders_flags_table_when_command_has_args() {
        let files = emit_docs(&sample_root());
        let publish = find_file(&files, "publish.mdx").expect("publish.mdx emitted");
        assert!(
            publish.content.contains("## Flags"),
            "publish has flags → `## Flags` section must appear; got:\n{}",
            publish.content,
        );
        assert!(
            publish.content.contains("--type"),
            "flag table must mention `--type`; got:\n{}",
            publish.content,
        );
        assert!(
            publish.content.contains("--data"),
            "flag table must mention `--data`; got:\n{}",
            publish.content,
        );
    }

    #[test]
    fn emit_docs_omits_flags_section_when_command_has_no_args() {
        let files = emit_docs(&sample_root());
        let status = find_file(&files, "status.mdx").expect("status.mdx emitted");
        assert!(
            !status.content.contains("## Flags"),
            "status has no args → `## Flags` section must be omitted; got:\n{}",
            status.content,
        );
    }

    #[test]
    fn emit_docs_renders_examples_section_from_after_long_help() {
        // Commands annotated with `#[command(after_long_help = "...")]`
        // (or the builder equivalent) surface that text under an
        // `## Examples` heading.
        let cmd = Command::new("nt").subcommand(
            Command::new("publish")
                .about("Publish events.")
                .after_long_help("$ nt publish --type x --data '{}'"),
        );
        let files = emit_docs(&cmd);
        let publish = find_file(&files, "publish.mdx").expect("publish.mdx emitted");
        assert!(
            publish.content.contains("## Examples"),
            "after_long_help present → `## Examples` section must appear; got:\n{}",
            publish.content,
        );
        assert!(
            publish
                .content
                .contains("$ nt publish --type x --data '{}'"),
            "examples body must include the annotation text verbatim; got:\n{}",
            publish.content,
        );
    }

    #[test]
    fn emit_docs_omits_examples_section_when_no_after_long_help() {
        let files = emit_docs(&sample_root());
        let publish = find_file(&files, "publish.mdx").expect("publish.mdx emitted");
        // Pin against accidentally emitting an empty Examples block.
        assert!(
            !publish.content.contains("## Examples"),
            "no after_long_help → `## Examples` section must be omitted; got:\n{}",
            publish.content,
        );
    }

    // ─── determinism ───────────────────────────────────────────────────────

    #[test]
    fn emit_docs_is_idempotent() {
        // Re-emitting against the same input must produce
        // byte-identical output — the release-tag workflow relies on
        // this for its diff-aware "no-op on unchanged" exit.
        let first = emit_docs(&sample_root());
        let second = emit_docs(&sample_root());
        assert_eq!(
            first, second,
            "two consecutive emit_docs calls must produce identical output",
        );
    }

    #[test]
    fn emit_docs_emits_subcommands_in_stable_alphabetical_order() {
        // Two top-level subcommands in non-alphabetical builder
        // order. The emitter must impose a stable order so a
        // builder-order change doesn't churn the docs PR.
        let cmd = Command::new("nt")
            .subcommand(Command::new("zebra").about("z."))
            .subcommand(Command::new("alpha").about("a."));
        let files = emit_docs(&cmd);
        let names: Vec<_> = files
            .iter()
            .map(|f| f.path.to_string_lossy().into_owned())
            .collect();
        let alpha_idx = names.iter().position(|n| n == "alpha.mdx");
        let zebra_idx = names.iter().position(|n| n == "zebra.mdx");
        match (alpha_idx, zebra_idx) {
            (Some(a), Some(z)) => assert!(
                a < z,
                "alpha must precede zebra in emitter output; got order: {names:?}",
            ),
            other => panic!("both files must be emitted; got {other:?} from {names:?}"),
        }
    }
}
