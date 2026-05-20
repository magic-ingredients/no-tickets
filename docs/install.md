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
> (`cargo publish` of `no-tickets`) is not yet shipped. The crate name
> `no-tickets` on crates.io is currently a placeholder at v0.0.0
> reserved by an unrelated author; `cargo install no-tickets --locked`
> will not install this binary today. Use one of the channels above.
> This section will be replaced with the working command when Task 8
> lands.

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
