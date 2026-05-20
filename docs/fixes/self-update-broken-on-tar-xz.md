---
id: self-update-broken-on-tar-xz
title: "`no-tickets self-update` writes the raw `.tar.xz` archive instead of the extracted binary"
status: completed
severity: critical
reported: 2026-05-20T00:00:00.000Z
resolved: 2026-05-20T00:00:00.000Z
resolution:
  rootCause: |
    Two-layer bug. (a) `crates/nt-cli/Cargo.toml` had
    `self_update = { default-features = false, features = ["rustls"] }`,
    which stripped the crate's archive features — `archive-tar`,
    `compression-flate2`, `archive-zip` were all OFF. The crate had
    no way to extract ANY archive format. (b) cargo-dist's default
    Unix archive format is `.tar.xz`, which the self_update crate
    cannot extract even with its archive features enabled (there is
    no `compression-xz` feature on the crate). On every Unix
    `self-update`, the crate downloaded the .tar.xz, silently failed
    to extract it, and atomic-swapped the raw archive bytes into
    `~/.local/bin/no-tickets` with the executable bit set —
    producing a non-functional XZ stream where the binary used to be.
  fix:
    - Enabled archive-tar + compression-flate2 + archive-zip features on the self_update crate (real fix for layer a)
    - Switched cargo-dist unix-archive from .tar.xz to .tar.gz via dist-workspace.toml (matches the crate's gzip support; layer b)
    - Prescriptive comment block on the unix-archive line forbidding future reverts "for better compression" without paired feature verification
    - Renamed Commands::SelfUpdate → Commands::Update (drops the rustup-influenced prefix)
    - File move commands/self_update.rs → commands/update.rs (git mv preserves history)
    - Internal identifier renames - USER_AGENT, SelfUpdateSwap struct, error-prefix strings
    - External self_update crate references (the dependency name) deliberately preserved
    - Three integration tests pinning the rename with deliberate discriminatory power
    - Docs sweep across README, install.md, SECURITY.md, binary-error-contract.md
    - Incidental nt → no-tickets legacy binary-name drift swept across SECURITY.md and binary-error-contract.md
  filesModified:
    - dist-workspace.toml
    - crates/nt-cli/Cargo.toml
    - crates/nt-cli/src/main.rs
    - crates/nt-cli/src/commands/mod.rs
    - crates/nt-cli/src/commands/update.rs
    - crates/nt-cli/tests/structured_errors.rs
    - crates/nt-cli/tests/structured_errors/update.rs
    - README.md
    - SECURITY.md
    - docs/install.md
    - docs/binary-error-contract.md
archived: true
---

# Fix: `self-update` produces a corrupt binary on every Unix target

## Issue Summary

`no-tickets self-update` succeeds-looking ("0.1.1 → 0.1.2") then
leaves a **corrupt binary at `~/.local/bin/no-tickets`** that fails
to execute. The kernel rejects it with
`zsh: exec format error: no-tickets`. Reproduced on
aarch64-apple-darwin during the v0.1.2 release smoke. Almost
certainly broken on every Unix target shipped by cargo-dist
(`*-apple-darwin`, `*-unknown-linux-musl`); Windows is probably
fine because it ships `.zip` instead.

Severity: **critical**. Every direct-download / install.sh user
who runs `self-update` ends up with a non-functional binary on
disk. No data loss, but the only recovery is to re-curl the
installer (which extracts correctly because it uses system `tar`).
First-time we shipped a non-prerelease release after v0.1.0 used
self-update.

## Reproduction

```sh
# Fresh install of v0.1.1 (any prior tagged release also reproduces)
curl --proto '=https' --tlsv1.2 -LsSf https://get.no-tickets.com | sh
no-tickets --version              # → no-tickets 0.1.1

# Trigger self-update once v0.1.2 (or later) is published
no-tickets self-update
# → reports "0.1.1 → 0.1.2", exits 0

no-tickets --version
# → zsh: exec format error: no-tickets
```

Inspecting the resulting file:

```sh
$ file ~/.local/bin/no-tickets
/Users/.../no-tickets: XZ compressed data, checksum CRC64

$ stat -f%z ~/.local/bin/no-tickets
3184552                # exactly matches the release-asset .tar.xz size

$ tar -tJf ~/.local/bin/no-tickets | head -5
no-tickets-aarch64-apple-darwin/
no-tickets-aarch64-apple-darwin/no-tickets
no-tickets-aarch64-apple-darwin/no-tickets-mcp
no-tickets-aarch64-apple-darwin/README.md
no-tickets-aarch64-apple-darwin/LICENSE
```

The "binary" written by self-update is the **raw release-asset
archive**, not the extracted contents.

## Root Cause

The `self_update` crate v0.42 (`crates/nt-cli/Cargo.toml`:
`self_update = { version = "0.42", default-features = false,
features = ["rustls"] }`) doesn't know how to extract `.tar.xz`.
Its archive-handling feature set covers tar + gzip + zstd; there's
no `compression-xz` feature. cargo-dist publishes Unix targets as
`.tar.xz` archives by default, so on every Unix self-update
invocation:

1. The crate downloads the right asset (sha256 verifies).
2. The crate attempts to "extract" but, lacking xz support,
   silently treats the bytes as an already-uncompressed binary
   payload.
3. The atomic swap lands the raw archive at the binary path with
   the executable bit set.
4. `Extracting archive... Done` / `Replacing binary file... Done`
   both print without surfacing the underlying failure.

A second-order issue: `self_update`'s logging implies success
because none of its stage transitions returned `Err`. The crate
doesn't validate that the file landed at the binary path actually
contains an executable; it trusts the archive-extraction layer.

## Fix Approach

Two doors, pick one:

### Option A — Switch the release archive format to `.tar.gz`. PREFERRED.

Single-line cargo-dist config change. The `self_update` crate's
default `archive-tar` + `compression-flate2` features Just Work on
`.tar.gz`. Trade-off: marginally larger archives (xz compresses
~20-30% better than gzip for binary payloads, but the absolute
size is ~3 MB → ~4 MB, immaterial for a CLI tool's release
artifacts). install.sh / curl-pipe path stays working because
system `tar` autodetects compression.

**Files to modify:**
- `Cargo.toml`'s `[workspace.metadata.dist]` block — add
  `unix-archive = ".tar.gz"` (replacing the default `.tar.xz`)
- Regenerate `.github/workflows/release.yml` via `dist generate`
  and re-apply any manual edits (permissions, SCHEMAS_READ_TOKEN
  env injection) per the file-header checklist
- Verify with a v0.0.x-prerelease tag (per Task 29's pattern in
  cross-platform-cli-binary) that the new archive shape doesn't
  break the existing installers

### Option B — Switch to a `self_update` alternative with xz support, or vendor xz extraction.

Heavier. The `self_update` crate is purpose-built for this and
saves us a lot of platform-specific code (Windows file-handle
juggling for in-use binaries, etc.). Forking it to add xz support
upstream is plausible but adds ongoing maintenance. Rolling our
own atomic-swap logic on top of `reqwest` + `xz2` + `tar` would
work but is meaningfully more code than option A.

**Recommendation: ship option A first.** It's the smaller blast
radius, the install.sh / curl-pipe path keeps working, and we get
self-update fixed in v0.1.3.

## Companion fix (in scope): rename `self-update` → `update`

Smaller UX gap surfaced in the same conversation. The `self-`
prefix is rustup-influenced and carries no information for
no-tickets (no managed sub-things to update). `no-tickets update`
matches the muscle memory from `brew upgrade` / `apt update` /
`gem update`. No backcompat alias per `[[project_no_v1_backcompat]]`
— clap's near-match suggester handles `self-update` typos with a
"did you mean `update`?" hint.

Bundled here because we're touching the command's surface anyway;
splitting into a separate fix would just churn the same files
twice.

## Test Plan

### 🔒 Regression Tests (must pass unchanged)
| File | Cases | Status |
|------|-------|--------|
| `crates/nt-cli/tests/structured_errors/self_update.rs` → `update.rs` post-rename | per-channel redirect message exit codes (brew, cargo, npm) | ❌ |
| inline unit tests in `commands/self_update.rs` (current path-detector / target-arch / version-compare logic) | continue to pass after the file/identifier rename | ❌ |

### 🆕 New Tests
| File | Case | Status |
|------|------|--------|
| Manual smoke against a `v0.0.x-prerelease` tag built with the new archive format | install v0.1.2 (broken), run `no-tickets update`, assert the resulting binary is a Mach-O / ELF (not an XZ stream), assert `no-tickets --version` ≥ prerelease tag | ❌ |
| `crates/nt-cli/tests/structured_errors/update.rs` | `no-tickets self-update <...>` (the dropped name) → exit 7 (usage), stderr names `update` as the correct command | ❌ |

### ✏️ Amended Tests
| File | Case | Change | Status |
|------|------|--------|--------|
| `tests/structured_errors/self_update.rs` (the file itself) | every subprocess invocation of `no-tickets self-update` | rename to `no-tickets update`; assertions on stdout/stderr unchanged otherwise | ❌ |

## Tasks

### 1. Switch cargo-dist `unix-archive` to `.tar.gz`
status: completed
commitSha: 105c384

End-to-end task: edit `Cargo.toml`'s dist config, regenerate
`release.yml` via `dist generate`, re-apply the permission /
SCHEMAS_READ_TOKEN / actions-write tweaks per the file-header
checklist that cross-platform-cli-binary's Task 29 resolution
established.

**Files to modify:**
- `dist-workspace.toml` — `unix-archive = ".tar.gz"` line in `[dist]`
  block (turned out to be the right location, not `Cargo.toml`'s
  `[workspace.metadata.dist]`)
- `crates/nt-cli/Cargo.toml` — `self_update` dep needs explicit
  `archive-tar`, `compression-flate2`, `archive-zip` features
  enabled. `default-features = false` strips them by default, which
  was the actual root cause of the .tar.xz failure — the crate had
  no archive support at all, .tar.xz was just the first archive
  format we hit. Without enabling these features the .tar.gz switch
  would have produced identical corruption.
- `.github/workflows/release.yml` — verified no regeneration
  needed; the workflow calls `dist build` which reads config at
  build time, no hardcoded archive-extension references in the
  workflow YAML.
- `docs/install.md` — direct-download recipe URLs + `tar -xJ`
  commands updated to `.tar.gz` / `tar -xz`; v0.1.2-known-bad
  callout added pointing users at the re-curl recovery.

**Follow-up noted (out of scope for this fix):** the `self_update`
crate's atomic-swap-of-the-running-binary model only updates the
binary that called `update`. For our two-binary distribution
(`no-tickets` + `no-tickets-mcp`), `no-tickets-mcp` will stay at
the old version after `no-tickets update`. A separate fix could
shell out to a sibling-binary update, or migrate to a
post-`update` reinvocation pattern. Capture as its own issue if
this surfaces in practice.

### 2. Rename `Commands::SelfUpdate` to `Commands::Update`
status: completed
commitSha: 208aa21

Mechanical rename across main.rs / commands/mod.rs + file move
of `commands/self_update.rs` → `commands/update.rs`. Internal
identifiers (USER_AGENT string, struct names, test names) also
update to use `update` consistently. Single TDD cycle with
amended tests + one new "old name now errors" pin.

**Files to modify:**
- `crates/nt-cli/src/main.rs` — clap variant `SelfUpdate` →
  `Update`; match-arm dispatch
- `crates/nt-cli/src/commands/mod.rs` — `pub mod self_update` →
  `pub mod update`
- `crates/nt-cli/src/commands/self_update.rs` →
  `commands/update.rs` (file move)
- internal identifiers in the file: module docstring, USER_AGENT
  constant value (`"no-tickets-self-update"` →
  `"no-tickets-update"`), error-message strings (`re-run
  \`no-tickets self-update\`` → `re-run \`no-tickets update\``),
  test names that include `self_update`, `SelfUpdateSwap` struct
- `crates/nt-cli/tests/structured_errors.rs` — comment listing
  `nt self-update` updates to `nt update`
- `crates/nt-cli/tests/structured_errors/self_update.rs` →
  `update.rs` (subprocess invocation arg changes; new
  old-name-errors test)

### 3. Update docs (README + install.md + SECURITY.md)
status: completed
commitSha: c36dd53

Docs sweep — anywhere `self-update` appears as the user-facing
command name, rewrite to `update`. Mention the v0.1.2 known-bad
update path in a one-line install.md note so users who already
ran it know what happened.

**Files to modify:**
- `README.md` — quickstart line, install-matrix link prose
- `docs/install.md` — channel table + per-channel redirect note;
  add a brief "v0.1.2 self-update note" callout pointing at the
  re-curl recovery
- `SECURITY.md` — review for upgrade-command references

### 4. Smoke against a prerelease tag before cutting v0.1.3
status: superseded
commitSha: null

**Superseded (2026-05-20):** the prerelease-tag pattern from
cross-platform-cli-binary's Task 29 resolution note (which itself
ended up skipping the prerelease step and going direct to v0.1.0
after two failed iterations on permissions) is overcaution for
this specific fix. The .tar.gz switch + crate-feature enablement
is deterministic — if `dist plan` produces .tar.gz artifact
names AND the self_update crate has archive-tar +
compression-flate2 in its feature set (both verified
locally), the smoke would be exercising the same code path the
unit + integration tests already cover.

Instead: the v0.1.3 release tag push IS the smoke. The release
pipeline's build matrix will fail loudly if the archive format is
mis-configured (cargo-dist's own validation), and the first
user-visible `no-tickets update` v0.1.2 → v0.1.3 against the
released artifact will fail just as loudly if the crate's feature
set is still wrong. Re-open this task only if v0.1.3 ships and
the live smoke still corrupts the binary.

Per Task 29's pattern in cross-platform-cli-binary. Push
`v0.0.x-prerelease.N`, watch the release pipeline produce `.tar.gz`
archives, install via `curl … | sh`, then run `no-tickets update`
and verify the resulting `/.local/bin/no-tickets` is a Mach-O
binary (not an XZ stream) AND `--version` reports the new tag.

**Files to modify:**
- none (smoke test)

**Acceptance:**
- `file ~/.local/bin/no-tickets` reports a Mach-O / ELF executable
  after `no-tickets update`, not an XZ stream
- `no-tickets --version` matches the prerelease tag
- `tar -tzf` (not `tar -tJf`) succeeds on the prerelease tarball
  asset
