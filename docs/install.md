# Installing no-tickets

The `no-tickets` release ships two binaries: `no-tickets` (the CLI) and
`no-tickets-mcp` (the MCP server). Every channel installs both together.

Supported platforms:

| OS | Architectures |
|----|--------------|
| macOS | Apple Silicon (aarch64), Intel (x86_64) |
| Linux | x86_64-musl, aarch64-musl (statically linked — runs on Alpine, `distroless/static`, glibc systems. NOT `distroless/base`, which expects libc resolution at runtime) |
| Windows | x86_64 |

## Install commands

### macOS and Linux — shell installer (`curl`)

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://get.no-tickets.com | sh
```

Installs both binaries to `~/.local/bin/`. Add that to your `PATH` if it
isn't already:

```bash
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.zshrc   # zsh
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc  # bash
```

The full URL the shorthand resolves to is
`https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-installer.sh`
— useful for environments where `get.no-tickets.com` is blocked.

### macOS and Linux — Homebrew

```bash
brew install magic-ingredients/tap/no-tickets
```

Installs both binaries via the magic-ingredients tap. `brew tap
magic-ingredients/tap` is implicit on the first install.

### Windows — PowerShell installer

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://get.no-tickets.com/installer.ps1 | iex"
```

Installs both binaries to `%USERPROFILE%\.local\bin\` and prepends that
directory to the user `PATH`. Restart the shell after install.

### Rust ecosystem — `cargo install`

> **Coming soon.** Task 8 of the cross-platform-cli-binary fix
> (`cargo publish` of the real binary) hasn't shipped yet. The crate
> name `no-tickets` on crates.io is currently a defensive placeholder
> at v0.0.0 — we own it so nobody else can squat the name; it just
> doesn't ship a working binary today. Use the shell installer or
> Homebrew above. When Task 8 lands, this section will be replaced
> with the working `cargo install no-tickets --locked` command.

### Build from source

If you can't `curl … | sh` (air-gapped, restricted runners) and don't
want to wait for the cargo channel, build from a clone:

```bash
git clone https://github.com/magic-ingredients/no-tickets.git
cd no-tickets
cargo install --path crates/nt-cli --locked
```

Installs `no-tickets` and `no-tickets-mcp` into `$CARGO_HOME/bin/`
(defaults to `~/.cargo/bin/`). Requires a Rust toolchain and network
access to crates.io for transitive dependencies. The build pulls a
sha256-verified schemas bundle from
`magic-ingredients/no-tickets-service` releases via `build.rs` — `gh`
CLI must be on PATH and authenticated to that repo (or the
`GH_TOKEN` env must be a PAT with `Contents:Read` on it).

### Direct tarball download

If you'd rather verify checksums and place the binaries yourself, download
the per-target tarball from the latest release and extract:

```bash
# macOS Apple Silicon
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-aarch64-apple-darwin.tar.gz \
  | tar -xz
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/

# macOS Intel
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-apple-darwin.tar.gz \
  | tar -xz
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/

# Linux x86_64 (static musl — runs anywhere)
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.gz \
  | tar -xz
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/

# Linux aarch64 (static musl)
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-aarch64-unknown-linux-musl.tar.gz \
  | tar -xz
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/
```

Each tarball ships with an `.sha256` neighbour for checksum verification.
The verifier looks for the file by name in the current directory, so run
it from wherever you downloaded the tarball:

```bash
cd /tmp   # or wherever you want to download
curl -LO https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.gz
curl -LO https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.gz.sha256
shasum -a 256 -c no-tickets-x86_64-unknown-linux-musl.tar.gz.sha256
```

Windows users: replace the `.tar.gz` URL with
`no-tickets-x86_64-pc-windows-msvc.zip` and extract into a directory on
your `PATH`.

> **v0.1.2 known-bad `self-update`:** the v0.1.2 release ships
> `.tar.xz` archives that the bundled `self_update` library can't
> extract — running `no-tickets self-update` on v0.1.2 leaves a
> non-executable XZ stream where the binary used to be. Recovery:
> re-run `curl … | sh` (system `tar` handles `.tar.xz`). v0.1.3 onward
> ships `.tar.gz`, and the subcommand is also renamed
> `self-update` → `update`; the updater works as designed from v0.1.3
> on.

## Verifying the install

```bash
no-tickets --version       # prints the release version
no-tickets-mcp --version   # prints the same version — the binaries ship in lockstep
no-tickets status          # prints auth + locally registered tokens as JSON
```

`no-tickets status` works without authentication; `no-tickets init` is the
next step to sign in.

## Updating

How `no-tickets` updates depends on which channel installed it.

| Channel | Update command |
|---------|---------------|
| `curl … \| sh` / direct download | `no-tickets update` |
| Homebrew | `brew upgrade no-tickets` |
| Direct tarball | re-download and re-install |

`no-tickets update` detects which install channel placed the binary
and prints the right command for that channel rather than running an
in-place swap that would conflict with the package manager. After a
Homebrew install, for example, `no-tickets update` prints
`no-tickets was installed via Homebrew. Run \`brew upgrade no-tickets\` to
update.` and exits cleanly.

## Why a binary?

Earlier versions of no-tickets shipped the CLI as `npx no-tickets …` (a Node
script in `@magic-ingredients/no-tickets`). The native binary replaces it for
three reasons:

- **No Node runtime required.** CI agents, container images, and machines
  without Node now install in seconds with a single `curl | sh`.
- **Cold-start cost.** Node's startup overhead on a per-event publish was
  ~150 ms; the Rust binary cold-starts in ~50 ms and supports a `--stream`
  mode for warm subprocess reuse (sub-millisecond per event).
- **Multi-channel distribution.** brew, cargo, scoop, install.sh, and direct
  download all serve the same artifact, so users install via their preferred
  tool.

The npm package today ships the TypeScript SDK only (for programmatic
use from JS / TS); the CLI and MCP server retired from npm with the
binary release. Phase 4 of the client roadmap (per
[ADR-0003](adr/0003-typescript-sdk-phase-4-survival.md)) reintroduces a
thin `/client` export that spawns the binary via the `--stream`
protocol — this sentence will be rewritten then.

## Using no-tickets in CI

CI runners usually install fresh on every job — package managers don't
fit, but `curl … | sh` does. Three things to handle in every recipe:

1. **Install** the binary via the shell installer.
2. **PATH** — the installer drops both binaries into `~/.local/bin`,
   which isn't on most CI runner's default PATH; each later step has
   to find them.
3. **Auth** — set `NO_TICKETS_TOKEN` from your CI secret store rather
   than running `no-tickets init` (which opens a browser).

Tokens are minted on a workstation via `no-tickets token add
<project> <token>` and the raw token value (`nt_push_…`) is what goes
in the secret store. The CLI reads `NO_TICKETS_TOKEN` before consulting
the local registry, so CI doesn't need a registry file.

### GitHub Actions

```yaml
jobs:
  publish-event:
    runs-on: ubuntu-latest
    steps:
      - name: Install no-tickets
        run: |
          curl --proto '=https' --tlsv1.2 -LsSf https://get.no-tickets.com | sh
          echo "$HOME/.local/bin" >> $GITHUB_PATH

      - name: Publish event
        env:
          NO_TICKETS_TOKEN: ${{ secrets.NO_TICKETS_TOKEN }}
        run: |
          no-tickets publish \
            --project my-project \
            --type ai.task.completed.v1 \
            --data '{"taskId":"${{ github.run_id }}","outcome":"success"}'
```

The `echo "$HOME/.local/bin" >> $GITHUB_PATH` step is mandatory — the
installer modifies shell rc files, but every Actions step runs in a
fresh shell that doesn't source them.

### GitLab CI

```yaml
.no-tickets-setup: &no-tickets-setup
  before_script:
    - curl --proto '=https' --tlsv1.2 -LsSf https://get.no-tickets.com | sh
    - export PATH="$HOME/.local/bin:$PATH"

publish-event:
  image: alpine:3.20
  variables:
    NO_TICKETS_TOKEN: $NO_TICKETS_TOKEN   # from CI/CD Variables
  <<: *no-tickets-setup
  script:
    - no-tickets publish --project my-project --type ai.task.completed.v1 --data '{"runId":"'"$CI_PIPELINE_ID"'"}'
```

The `<<: *no-tickets-setup` YAML anchor lets you reuse the install
across jobs without duplication. Drop the anchor + paste the two
`before_script` lines inline if your YAML pipeline doesn't use anchors.

### Generic shell (CircleCI, Bitbucket Pipelines, Jenkins, Drone, …)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://get.no-tickets.com | sh
export PATH="$HOME/.local/bin:$PATH"

# Provider sets NO_TICKETS_TOKEN from the secret store at job start.
no-tickets publish \
  --project my-project \
  --type ai.task.completed.v1 \
  --data "{\"runId\":\"$BUILD_ID\"}"
```

Each provider has its own secret-store mechanism (CircleCI contexts,
Bitbucket repository variables, Jenkins credentials, Drone secrets);
the env-var contract on the binary is the same.

### Verifying the install in CI

Add a `no-tickets --version` step after install so a broken release
fails the job at install time, not at first publish:

```yaml
- run: no-tickets --version    # surfaces the release version into the log
```

This also covers the case where `~/.local/bin` didn't make it onto
PATH — `no-tickets --version` will exit "command not found" loudly
rather than letting a downstream publish step fail with a confusing
error.

## Coming soon

- **deb / rpm** repositories for apt / yum users (tracked in
  [`docs/fixes/deb-rpm-packaging.md`](fixes/deb-rpm-packaging.md);
  waiting on demand signal).
- **`cargo install no-tickets`** (tracked as Task 8 of
  [`docs/fixes/cross-platform-cli-binary.md`](fixes/cross-platform-cli-binary.md);
  the crates.io name is currently a defensive placeholder we own).
- **Per-language wrappers** (Python, Go, TS) for in-process programmatic
  publishing without spawning the binary per event — Phase 4 of the
  client roadmap; depends on
  [`docs/fixes/stream-mode.md`](fixes/stream-mode.md).

Windows-side note: a Scoop manifest was considered and ruled out
(see Task 34 of `cross-platform-cli-binary`). The PowerShell installer
+ direct ZIP download cover day-one Windows users. If a package-manager
channel is needed, `winget` is the strategic pick — open a task at
that point.

## Reporting install issues

If the installer fails on your platform, please file an issue at
[github.com/magic-ingredients/no-tickets/issues](https://github.com/magic-ingredients/no-tickets/issues)
with the OS / arch / install channel + the install command's output.
