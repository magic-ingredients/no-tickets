//! `no-tickets internal …` — hidden subcommands intended for build /
//! release tooling, not human use. Each member of this group is
//! marked `hide = true` so it never appears in public `--help` output.
//!
//! Today: just `generate-docs`, the MDX emitter that walks the Clap
//! tree and writes one page per command into the docs-site repo's
//! `cli-reference/` tree.

pub mod generate_docs;
