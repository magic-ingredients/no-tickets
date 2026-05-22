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

use std::fmt::Write as _;
use std::fs;
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
pub fn emit_docs(root: &clap::Command) -> Vec<EmittedFile> {
    let mut files = Vec::new();
    let mut subs: Vec<&clap::Command> = root.get_subcommands().collect();
    // Stable alphabetical order — closes the door on Clap returning
    // subcommands in registration order (which would churn the docs
    // PR every time a new flag landed mid-list).
    subs.sort_by_key(|c| c.get_name());
    for sub in subs {
        emit_recursive(sub, root.get_name(), &[], &mut files);
    }
    files
}

/// Recursive walker. `binary_name` is the root command's name (e.g.
/// `no-tickets`) — used to build the synopsis line. `parents` is the
/// chain of names between the root and `cmd`, used for the full
/// invocation path in titles and the relative output path.
fn emit_recursive(
    cmd: &clap::Command,
    binary_name: &str,
    parents: &[&str],
    out: &mut Vec<EmittedFile>,
) {
    if cmd.is_hide_set() {
        return;
    }
    let mut path = parents.to_vec();
    path.push(cmd.get_name());

    out.push(EmittedFile {
        path: page_path(&path),
        content: render_page(cmd, binary_name, &path),
    });

    let mut subs: Vec<&clap::Command> = cmd.get_subcommands().collect();
    subs.sort_by_key(|c| c.get_name());
    for sub in subs {
        emit_recursive(sub, binary_name, &path, out);
    }
}

fn page_path(path: &[&str]) -> PathBuf {
    PathBuf::from(format!("{}.mdx", path.join("/")))
}

/// Render the MDX body for one command.
fn render_page(cmd: &clap::Command, binary_name: &str, path: &[&str]) -> String {
    let title = path.join(" ");
    let description_raw = cmd
        .get_about()
        .map(|s| s.to_string())
        .unwrap_or_default()
        // Frontmatter values must be a single line; collapse newlines
        // defensively even though `about` is typically one line.
        .replace('\n', " ");
    let mut out = String::new();
    out.push_str("---\n");
    let _ = writeln!(out, "title: {title}");
    // Quote the description as a YAML double-quoted scalar so colons,
    // hash chars, leading dashes, and embedded quotes in command help
    // text don't produce invalid frontmatter. The escape rules below
    // cover `"` and `\`; everything else is safe inside double quotes.
    let _ = writeln!(out, "description: {}", yaml_double_quote(&description_raw));
    out.push_str("---\n\n");

    out.push_str("## Usage\n\n");
    out.push_str("```bash\n");
    let _ = writeln!(out, "{}", synopsis(cmd, binary_name, path));
    out.push_str("```\n");

    let flag_rows = flag_rows(cmd);
    if !flag_rows.is_empty() {
        out.push_str("\n## Flags\n\n");
        out.push_str("| Flag | Short | Default | Description |\n");
        out.push_str("|---|---|---|---|\n");
        for row in flag_rows {
            out.push_str(&row);
            out.push('\n');
        }
    }

    if let Some(examples) = cmd.get_after_long_help() {
        let body = examples.to_string();
        let trimmed = body.trim_end_matches('\n');
        if !trimmed.trim().is_empty() {
            out.push_str("\n## Examples\n\n");
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    out
}

/// Build the synopsis line — `<binary> <command> [OPTIONS]`. Conditionally
/// required flags (`required_unless_present`) and explicit `--flag` lists
/// vary by command and are best documented in the `## Flags` table; the
/// synopsis stays minimal so a reader gets the invocation shape without
/// false precision.
fn synopsis(cmd: &clap::Command, binary_name: &str, path: &[&str]) -> String {
    let mut out = format!("{binary_name} {}", path.join(" "));
    if has_visible_flags(cmd) {
        out.push_str(" [OPTIONS]");
    }
    out
}

fn flag_rows(cmd: &clap::Command) -> Vec<String> {
    let mut args: Vec<&clap::Arg> = cmd
        .get_arguments()
        .filter(|a| is_visible_user_flag(a))
        .collect();
    args.sort_by_key(|a| a.get_long().unwrap_or("").to_string());
    args.iter().map(|a| render_flag_row(a)).collect()
}

/// Whether `cmd` carries at least one user-visible flag (skipping
/// `--help` / `--version`, hidden args, and positionals). Used to gate
/// the `[OPTIONS]` synopsis suffix and the `## Flags` section
/// emission.
fn has_visible_flags(cmd: &clap::Command) -> bool {
    cmd.get_arguments().any(is_visible_user_flag)
}

/// Filter: keep only the args we want in the docs table — non-hidden,
/// non-positional, and not auto-injected by Clap (`--help` /
/// `--version`). Discriminating on `ArgAction` is more robust than
/// matching `get_id() == "help"` because users could shadow the ids;
/// the action stays Help / Version regardless.
fn is_visible_user_flag(a: &clap::Arg) -> bool {
    if a.is_hide_set() || a.is_positional() {
        return false;
    }
    !matches!(
        a.get_action(),
        clap::ArgAction::Help
            | clap::ArgAction::HelpShort
            | clap::ArgAction::HelpLong
            | clap::ArgAction::Version
    )
}

fn render_flag_row(arg: &clap::Arg) -> String {
    // `flag_rows` filters out positional args, so every arg arriving
    // here has a `--long`. The defensive `None` arm below stays so a
    // future filter change (e.g. allowing short-only flags) doesn't
    // panic at runtime.
    let long = match arg.get_long() {
        Some(l) => {
            let value = value_placeholder(arg)
                .map(|v| format!(" <{v}>"))
                .unwrap_or_default();
            format!("`--{l}{value}`")
        }
        None => String::new(),
    };
    let short = arg
        .get_short()
        .map(|c| format!("`-{c}`"))
        .unwrap_or_default();
    let default = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let description = arg
        .get_help()
        .map(|s| s.to_string().replace('\n', " "))
        .unwrap_or_default();
    format!(
        "| {} | {} | {} | {} |",
        escape_pipe(&long),
        escape_pipe(&short),
        escape_pipe(&default),
        escape_pipe(&description),
    )
}

/// Returns the value placeholder for an arg's `--flag <VALUE>` form,
/// or `None` for bool / counting flags that don't take a value. Bool
/// flags (`SetTrue` / `SetFalse`) and counters (`Count`) are presence-
/// only — appending a `<NAME>` to them is a category error that broke
/// the original implementation on `--quiet`.
fn value_placeholder(arg: &clap::Arg) -> Option<String> {
    match arg.get_action() {
        clap::ArgAction::SetTrue | clap::ArgAction::SetFalse | clap::ArgAction::Count => None,
        _ => arg
            .get_value_names()
            .and_then(|v| v.first())
            .map(|s| s.to_string()),
    }
}

/// Escape `|` inside a markdown table cell. A raw pipe ends the cell
/// early, breaking the row alignment for every subsequent column. GFM
/// table syntax uses `\|` for a literal pipe.
fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Quote a string as a YAML double-quoted scalar. Escapes `\` and `"`;
/// other characters are safe inside double quotes (no special handling
/// needed for `:`, `#`, leading `-`, etc.).
fn yaml_double_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// CLI entrypoint for `no-tickets internal generate-docs <target>`.
/// `root` is the Clap `Command` from `<Cli as CommandFactory>::command()`
/// in `main.rs` — passed in (rather than re-built here) so the emitter
/// sees the exact same surface as live argument parsing and the `Cli`
/// struct stays private to `main.rs`.
pub fn run(root: &clap::Command, target: &Path) -> i32 {
    let files = emit_docs(root);
    for file in &files {
        let full = target.join(&file.path);
        if let Some(parent) = full.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("create_dir_all({parent:?}): {e}");
                return 1;
            }
        }
        if let Err(e) = fs::write(&full, &file.content) {
            eprintln!("write({full:?}): {e}");
            return 1;
        }
    }
    println!("emitted {} files to {}", files.len(), target.display());
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
        // Description is always YAML-quoted so colons / hash chars in
        // command help text don't break the frontmatter parse.
        assert!(
            publish.content.contains("description: \"Publish events.\""),
            "description must be YAML-quoted from the command's `about` text; got:\n{}",
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

    // ─── escaping + safety ─────────────────────────────────────────────────

    #[test]
    fn emit_docs_escapes_pipe_in_flag_help_text() {
        // A raw `|` inside a `--help` description ends the markdown
        // table cell early, breaking alignment for every subsequent
        // column. The renderer must escape `|` as `\|`.
        let cmd = Command::new("nt").subcommand(
            Command::new("foo")
                .about("Foo command.")
                .arg(Arg::new("mode").long("mode").help("one of: a | b | c")),
        );
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        assert!(
            !foo.content.contains("a | b | c"),
            "raw `|` in help text must be escaped — table-cell breakage; got:\n{}",
            foo.content,
        );
        assert!(
            foo.content.contains("a \\| b \\| c"),
            "pipe must be rendered as `\\|`; got:\n{}",
            foo.content,
        );
    }

    #[test]
    fn emit_docs_yaml_quotes_description_so_colons_dont_break_frontmatter() {
        // A description like `Foo: do the X thing` would, unquoted,
        // parse as a YAML mapping. Wrap in double quotes so colons,
        // hash chars, and leading dashes stay literal.
        let cmd =
            Command::new("nt").subcommand(Command::new("foo").about("Foo: does the thing #urgent"));
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        assert!(
            foo.content
                .contains("description: \"Foo: does the thing #urgent\""),
            "description must be YAML-quoted; got:\n{}",
            foo.content,
        );
    }

    #[test]
    fn emit_docs_yaml_escapes_double_quotes_inside_description() {
        let cmd =
            Command::new("nt").subcommand(Command::new("foo").about(r#"Quote-using: "the" thing"#));
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        assert!(
            foo.content
                .contains(r#"description: "Quote-using: \"the\" thing""#),
            "inner double quotes must be `\\\"`-escaped; got:\n{}",
            foo.content,
        );
    }

    #[test]
    fn emit_docs_bool_flag_renders_without_value_placeholder() {
        // `--quiet` with `ArgAction::SetTrue` is a presence-only flag;
        // appending `<QUIET>` would falsely suggest it takes a value.
        let cmd = Command::new("nt").subcommand(
            Command::new("foo").about("Foo.").arg(
                Arg::new("quiet")
                    .long("quiet")
                    .action(clap::ArgAction::SetTrue),
            ),
        );
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        assert!(
            foo.content.contains("`--quiet`"),
            "bool flag must render the long flag name; got:\n{}",
            foo.content,
        );
        assert!(
            !foo.content.contains("--quiet <"),
            "bool flag must NOT render a value placeholder; got:\n{}",
            foo.content,
        );
        assert!(
            !foo.content.contains("<QUIET>"),
            "bool flag must NOT show the auto-derived value name; got:\n{}",
            foo.content,
        );
    }

    #[test]
    fn emit_docs_filters_out_auto_injected_help_and_version_rows() {
        // Clap auto-injects `--help` (and `--version` if `version` is
        // set on the root). Both rows would pollute every command's
        // flag table for zero useful signal. Filter them out by
        // ArgAction — robust to id-shadowing.
        let cmd = Command::new("nt").version("0.0.0").subcommand(
            Command::new("foo")
                .about("Foo.")
                .arg(Arg::new("name").long("name")),
        );
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        assert!(
            foo.content.contains("`--name`"),
            "user flag must appear in the table; got:\n{}",
            foo.content,
        );
        assert!(
            !foo.content.contains("`--help`"),
            "auto-injected --help must NOT appear in flag table; got:\n{}",
            foo.content,
        );
        assert!(
            !foo.content.contains("`--version`"),
            "auto-injected --version must NOT appear in flag table; got:\n{}",
            foo.content,
        );
    }

    #[test]
    fn emit_docs_flag_table_renders_default_value_in_default_column() {
        let cmd = Command::new("nt").subcommand(
            Command::new("foo")
                .about("Foo.")
                .arg(Arg::new("retries").long("retries").default_value("3")),
        );
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        // The Default column is the 3rd `|`-separated cell after the
        // pipe table opens. Use a structural check: locate the row
        // containing `--retries`, split on `|`, and assert the default
        // cell holds `3`.
        let row = foo
            .content
            .lines()
            .find(|l| l.contains("--retries"))
            .expect("row for --retries present");
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        // Cells layout: ["", "`--retries`", "", "3", "", ""] (with
        // empty leading + trailing from the surrounding `|`s).
        assert!(
            cells.contains(&"3"),
            "Default column must hold `3`; row was: {row:?} cells: {cells:?}",
        );
    }

    // ─── synopsis branches ──────────────────────────────────────────────────

    #[test]
    fn emit_docs_synopsis_omits_options_suffix_for_flagless_commands() {
        let cmd = Command::new("nt").subcommand(Command::new("ping").about("Ping."));
        let files = emit_docs(&cmd);
        let ping = find_file(&files, "ping.mdx").expect("ping.mdx emitted");
        assert!(
            !ping.content.contains("[OPTIONS]"),
            "flagless command must NOT show `[OPTIONS]` in synopsis; got:\n{}",
            ping.content,
        );
    }

    #[test]
    fn emit_docs_synopsis_appends_options_for_commands_with_flags() {
        let cmd = Command::new("nt").subcommand(
            Command::new("foo")
                .about("Foo.")
                .arg(Arg::new("name").long("name")),
        );
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        assert!(
            foo.content.contains("nt foo [OPTIONS]"),
            "command with flags must show `[OPTIONS]`; got:\n{}",
            foo.content,
        );
    }

    // ─── examples whitespace ────────────────────────────────────────────────

    #[test]
    fn emit_docs_examples_section_normalises_trailing_newlines() {
        // Multi-line `after_long_help` bodies sometimes end with
        // multiple `\n`s. The renderer must collapse trailing
        // whitespace to a single newline so the next section's heading
        // doesn't pick up an empty line in front of it (the markdown
        // renderer treats a missing blank line as run-on text).
        let cmd = Command::new("nt").subcommand(
            Command::new("foo")
                .about("Foo.")
                .after_long_help("$ nt foo\n$ nt foo --x\n\n\n"),
        );
        let files = emit_docs(&cmd);
        let foo = find_file(&files, "foo.mdx").expect("foo.mdx emitted");
        // The Examples body should end with exactly one `\n`, no
        // trailing blank lines.
        assert!(
            foo.content.ends_with("$ nt foo --x\n"),
            "trailing newlines must be collapsed to one; got:\n{}",
            foo.content,
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
