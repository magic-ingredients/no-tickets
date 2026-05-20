# Installing no-tickets

The `no-tickets` release ships two binaries: `no-tickets` (the CLI) and
`no-tickets-mcp` (the MCP server). Every channel installs both together.

Supported platforms:

| OS | Architectures |
|----|--------------|
| macOS | Apple Silicon (aarch64), Intel (x86_64) |
| Linux | x86_64-musl, aarch64-musl (statically linked — runs on Alpine, distroless, glibc systems) |
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

### Direct tarball download

If you'd rather verify checksums and place the binaries yourself, download
the per-target tarball from the latest release and extract:

```bash
# macOS Apple Silicon
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-aarch64-apple-darwin.tar.xz \
  | tar -xJ
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/

# macOS Intel
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-apple-darwin.tar.xz \
  | tar -xJ
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/

# Linux x86_64 (static musl — runs anywhere)
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.xz \
  | tar -xJ
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/

# Linux aarch64 (static musl)
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-aarch64-unknown-linux-musl.tar.xz \
  | tar -xJ
sudo install -m 755 no-tickets no-tickets-mcp /usr/local/bin/
```

Each tarball ships with an `.sha256` neighbour for checksum verification.
The verifier looks for the file by name in the current directory, so run
it from wherever you downloaded the tarball:

```bash
cd /tmp   # or wherever you want to download
curl -LO https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.xz
curl -LO https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.xz.sha256
shasum -a 256 -c no-tickets-x86_64-unknown-linux-musl.tar.xz.sha256
```

Windows users: replace the `.tar.xz` URL with
`no-tickets-x86_64-pc-windows-msvc.zip` and extract into a directory on
your `PATH`.

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
| `curl … \| sh` / direct download | `no-tickets self-update` |
| Homebrew | `brew upgrade no-tickets` |
| Direct tarball | re-download and re-install |

`no-tickets self-update` detects which install channel placed the binary
and prints the right command for that channel rather than running an
in-place swap that would conflict with the package manager. After a
Homebrew install, for example, `no-tickets self-update` prints
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

The npm package now ships the TypeScript SDK only (for programmatic use from
JS / TS); the CLI and MCP server retired from npm with the binary release.

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

- **Scoop** (Windows package manager) — separate from cargo-dist's installer
  set, currently in progress. Use the PowerShell installer or direct
  download in the meantime.
- **deb / rpm** repositories for apt / yum users.
- **Per-language wrappers** (Python, Go, TS) for in-process programmatic
  publishing without spawning the binary per event.

## Reporting install issues

If the installer fails on your platform, please file an issue at
[github.com/magic-ingredients/no-tickets/issues](https://github.com/magic-ingredients/no-tickets/issues)
with the OS / arch / install channel + the install command's output.
