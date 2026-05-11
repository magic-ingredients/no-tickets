# Rust spike notes — `cross-platform-cli-binary` Task 1

Findings from porting `nt status` to Rust as a toolchain spike before
committing to the full CLI rewrite (Tasks 4–5 in the fix doc).

**Commits in this spike:** `38defa7` → `d6814eb` → `3b8e15c` → `9cdeb2d` →
`3578250` → `80f18ea`. Six commits, 30 passing tests, single command
end-to-end.

## Toolchain choices that landed

All from the crate audit in the fix doc; the spike confirms each one
holds up at parity work.

| Concern | Crate | Notes from the spike |
|---|---|---|
| CLI parsing | `clap` (derive) | `#[arg(long, global = true)]` makes a flag work both before and after the subcommand — needed for TS argv parity (`nt --profile X status` ≡ `nt status --profile X`). Forces the test to pin both forms; both pass. |
| JSON in/out | `serde` + `serde_json` | Field order on the wire is governed by struct-field declaration order; no `preserve_order` feature needed for output. For the `Available: a, b` hint where on-disk profile insertion order matters, `serde_json::Map` is BTreeMap-backed by default — see "Surprises" below. |
| JSON map preserving insertion order | `indexmap` (with `serde` feature) | Required for any config map where on-disk key order is user-visible. Don't use `BTreeMap` for that case (alphabetises) and don't use `HashMap` (non-deterministic). |
| ISO 8601 expiry parsing | `time` (parsing feature only) | Drop `formatting` and `macros` features — not needed for parse-only consumers. `OffsetDateTime::parse(s, &Iso8601::DEFAULT)` handles `YYYY-MM-DDTHH:MM:SS.sssZ` from JS `Date.toISOString()` cleanly. |
| URL validation | `url` | `Url::parse` + scheme check + non-empty-host requirement is the right replacement for a `starts_with("https://")` prefix check. Will be transitively present via `reqwest` later anyway. |
| Test harness | `assert_cmd` + `predicates` + `tempfile` | Standard. `assert_cmd::Command::env_remove` plus `tempfile::tempdir()` give complete env-and-fs isolation. Use `predicate::str::contains(...).not()` for negative assertions. |
| Mutation testing | `cargo-mutants` | **Not Stryker** — Stryker (this repo's `stryker.config.mjs`) is wired for the TS package via `@stryker-mutator/vitest-runner` and doesn't support Rust. `cargo-mutants` is the Rust standard; `cargo install cargo-mutants` then `cargo mutants -f <files>`. Spike result: 19 killed / 1 equivalent (genuine) / 4 unviable on the changed files. |

## Module layout

```
crates/nt-cli/src/
├── main.rs          clap entry, dispatches to subcommands
├── home.rs          NO_TICKETS_HOME > HOME (Unix) / USERPROFILE (Windows)
├── credentials.rs   load ~/.notickets/credentials (shape + expiry validated)
├── auth.rs          env-token > credentials-file, push/session/unknown
├── urls.rs          --profile > env-vars (pair-validated) > defaults
└── status.rs        status command: URLs first, then auth, JSON to stdout
```

Responsibilities are cleanly separated — each module reflects one TS source
file from `src/sdk/`. The adversarial reviewer flagged in 9cdeb2d that
`urls.rs` carries the config-file I/O, which will need to be shared with
future commands (`init`, `project`, `connect`). **Action for Task 4:**
extract `config.rs` covering config-file read/parse/persist before adding
the second subcommand that touches it.

## Surprises

1. **BTreeMap silently re-orders profiles.** Initial impl used
   `BTreeMap<String, ProfileConfig>` for the `profiles` field of
   `config.json`. The "Available: …" hint emerged alphabetised. TS uses
   `Object.keys()` which is insertion order. Fixed with `IndexMap`
   (commit `3578250`); test pinned (commit `80f18ea`). Lesson: any
   on-disk map whose ordering is user-visible needs `IndexMap`.

2. **`fs::read_to_string` IO errors are not "invalid JSON".** Brief
   intermediate state mapped `io::ErrorKind::InvalidData` to a JSON
   parse error variant. Wrong: `InvalidData` from `read_to_string`
   means non-UTF-8 bytes, not malformed JSON. All `read_to_string`
   errors → "could not be read" is the right call; `serde_json::from_str`
   handles actual parse failures separately. Fixed in `80f18ea`.

3. **`starts_with("https://")` is not URL validation.** Accepts
   `https://` (empty host), `http:// nope` (space), embedded newlines,
   etc. Use `url::Url::parse` plus a `host_str().is_some_and(non_empty)`
   check. Pinned by `status_profile_https_without_host_is_invalid`.

4. **`println!` panics on broken pipe.** Default behaviour when
   stdout closes (e.g. `nt status | head -n 0`). Use `writeln!` against
   locked stdout with explicit `ErrorKind::BrokenPipe` handling.

5. **`home::home_dir()` failure must be a real error, not a panic.**
   The initial draft used `.expect("home dir resolvable")` for the URL
   path; with `NO_TICKETS_HOME`, `HOME`, and `USERPROFILE` all unset
   the binary would crash with a Rust stacktrace. Surface as
   `UrlError::HomeUnresolvable`. Pinned by
   `status_profile_with_no_home_resolvable_errors_gracefully`.

6. **Adversarial review fires after every `feat:`/`refactor:` commit
   on a tracked task.** Plan for ~2–3 iterations per task: write
   tests, implement, review, refactor, review again. Budget for it.

## Deliberate TS divergence

**Unparseable `expiresAt` is treated as not-authenticated.**

The TS impl computes `new Date(parsed.expiresAt).getTime()` which returns
`NaN` for unparseable strings; `NaN <= Date.now()` is `false` in JS, so
TS accidentally accepts the credential. The Rust port rejects it. This
is a designed behaviour, not parity — pinned by
`status_credentials_unparseable_expires_at_is_not_authenticated`. Worth
back-porting to TS during the broader rewrite.

## Performance

Cold cargo-build (debug): ~7.6 s on M1 Max, 71 dependencies. With release
optimisations the binary is ~3.5 MB stripped. Cold-start latency under
`cargo run` is dominated by load time; the actual binary cold-starts
in ~5 ms — well inside the fix doc's sub-50 ms target.

## Recommendations for Task 4 (full CLI port)

1. **Extract `config.rs`** before adding the second subcommand that
   touches `~/.notickets/config.json`. Currently lives in `urls.rs`.

2. **Add a `ProfileFileUnreadable` test** with a real fs failure
   (directory at the config path, or file with no read permission)
   to pin the IO-vs-JSON split. The IO-classification fix in `80f18ea`
   is correct by inspection but unpinned.

3. **Backport the unparseable-`expiresAt` fix to TS** as a separate
   commit on the same fix — defensible parity bug fix.

4. **Audit every `.expect()` / `.unwrap()`** before reaching
   user-facing paths. Each one is a potential Rust-stacktrace crash;
   convert to typed errors with `user_message()` per the existing
   `UrlError` pattern.

5. **Plan for `cargo-mutants` budget.** Mutation review is mandatory
   per the fix pipeline. Each command (`init`, `publish`, `project`,
   `validate`, `connect`, `token`) will need ~30–60 s of mutation
   runtime. Roughly 1–2 minutes per task.

6. **Consider extracting a `cli-test-support` dev-dep crate** when
   the second subcommand's test file starts duplicating the
   `isolate()` / `write_credentials()` / `write_config()` helpers.

7. **For `--profile` resolution alongside other commands**, the
   global-arg pattern (clap `global = true`) holds up. Don't make
   `--profile` per-subcommand or argv-position parity breaks.

## What the spike does NOT validate

- **MCP-server-side via `rmcp`.** Task 2 spike covers that — separate
  exercise.
- **HTTP transport via `reqwest` + `rustls`.** `nt status` makes no
  network call; the spike validates auth/URL plumbing only. Will be
  exercised by Task 4 commands (`publish`, `validate`).
- **Cross-compile.** Spike built for the host (`aarch64-apple-darwin`)
  only. `cargo-zigbuild` / `cargo-dist` work belongs to Task 6.
- **`build.rs` JSON Schema bundle integration.** Task 3 separately.

## Verdict

Toolchain is solid. No toolchain-level surprises that change the
plan. Proceed with Task 2 (`rmcp` spike) in parallel with Task 4
(full CLI port).
