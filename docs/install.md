# Installing no-tickets

The `no-tickets` release ships two binaries: `nt` (the CLI) and `nt-mcp`
(the MCP server). Every channel installs both together.

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

```bash
cargo install no-tickets --locked
```

Installs both binaries to `$CARGO_HOME/bin/` (defaults to `~/.cargo/bin/`).
Requires a Rust toolchain on your machine and builds from source — the other
channels ship pre-built binaries and are faster.

### Direct tarball download

If you'd rather verify checksums and place the binaries yourself, download
the per-target tarball from the latest release and extract:

```bash
# macOS Apple Silicon
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-aarch64-apple-darwin.tar.xz \
  | tar -xJ
sudo install -m 755 nt nt-mcp /usr/local/bin/

# macOS Intel
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-apple-darwin.tar.xz \
  | tar -xJ
sudo install -m 755 nt nt-mcp /usr/local/bin/

# Linux x86_64 (static musl — runs anywhere)
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-x86_64-unknown-linux-musl.tar.xz \
  | tar -xJ
sudo install -m 755 nt nt-mcp /usr/local/bin/

# Linux aarch64 (static musl)
curl -L https://github.com/magic-ingredients/no-tickets/releases/latest/download/no-tickets-aarch64-unknown-linux-musl.tar.xz \
  | tar -xJ
sudo install -m 755 nt nt-mcp /usr/local/bin/
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
nt --version       # prints the release version
nt-mcp --version   # prints the same version — the binaries ship in lockstep
nt status          # prints auth + locally registered tokens as JSON
```

`nt status` works without authentication; `nt init` is the next step to
sign in.

## Updating

How `nt` updates depends on which channel installed it.

| Channel | Update command |
|---------|---------------|
| `curl … \| sh` / direct download | `nt self-update` |
| Homebrew | `brew upgrade no-tickets` |
| `cargo install` | `cargo install --force no-tickets` |
| Direct tarball | re-download and re-install |

`nt self-update` detects which install channel placed the binary and prints
the right command for that channel rather than running an in-place swap that
would conflict with the package manager. After a Homebrew install, for
example, `nt self-update` prints `nt was installed via Homebrew. Run
\`brew upgrade no-tickets\` to update.` and exits cleanly.

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
