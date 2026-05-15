# no-tickets

Ticketless project management for AI teams. The `nt` CLI pushes
project-state events from your repo to a hosted dashboard; AI agents are
first-class and show up alongside human work.

## Install

```bash
# macOS / Linux
curl -fsSL https://get.no-tickets.com | sh

# Homebrew (macOS / Linux)
brew install magic-ingredients/tap/no-tickets

# Windows (PowerShell)
powershell -ExecutionPolicy ByPass -c "irm https://get.no-tickets.com/installer.ps1 | iex"

# Rust ecosystem
cargo install no-tickets --locked
```

Ships two binaries: `nt` (the CLI) and `nt-mcp` (the MCP server for use with
Claude Code and other MCP-capable agents). The single install command lands
both on your PATH.

Full install matrix (per-target tarballs, sha256 verification, self-update
behaviour per channel) lives at [docs/install.md](docs/install.md).

## Quickstart

```bash
nt init                                # browser auth + project setup
nt publish --type ai.task.completed.v1 \
           --data '{"taskId":"123","outcome":"success"}'
nt status                              # show auth + project state
nt validate                            # validate the .notickets/ directory
nt self-update                         # for install.sh / direct-download installs
```

## What is no-tickets?

A project management platform for teams building with AI agents. Instead of
manually creating and updating tickets, your dev tools push progress to a
dashboard automatically.

- **Developers** push project state from their repos
- **PMs** see real-time progress on a hosted dashboard
- **AI agents** are first-class — their work shows up alongside human work

## The `.notickets/` format

An open markdown spec for describing work:

```
.notickets/
├── user-auth/          # epic directory
│   ├── epic.md         # epic definition
│   ├── email-signup.md # feature
│   └── oauth-login.md  # feature
└── payments/
    ├── epic.md
    └── stripe.md
```

Each file uses YAML frontmatter + markdown. See [SPEC.md](SPEC.md) for the full
format specification.

## TypeScript SDK

For programmatic use from JavaScript / TypeScript (e.g., from tiny-brain or
custom tooling):

```bash
npm install @magic-ingredients/no-tickets
```

```typescript
import { computeState, computeDiff, parseFrontmatter } from '@magic-ingredients/no-tickets/sdk';
import type { FeatureState, Phase, StateSnapshot } from '@magic-ingredients/no-tickets/types';
```

The npm package ships the SDK only. The CLI and MCP server live in the
native binary above — `npx no-tickets` is no longer supported; install
the binary via one of the channels above.

## Works with tiny-brain

[tiny-brain](https://github.com/magic-ingredients/tiny-brain) is an open-source
Claude Code plugin that adds TDD enforcement, quality analysis, and adversarial
code review. When used with no-tickets, it also pushes telemetry data (model
usage, phase compliance, quality scores) that powers the engineering dashboard.

tiny-brain is optional — no-tickets works with any tool.

## Documentation

- Install matrix and self-update behaviour: [docs/install.md](docs/install.md)
- `.notickets/` format spec: [SPEC.md](SPEC.md)
- Hosted docs: [docs.no-tickets.com](https://docs.no-tickets.com)

## Contributing

The repo is a multi-language workspace: the CLI / MCP server are in Rust under
`crates/`, the SDK is in TypeScript at the root.

```bash
# TypeScript side (SDK + repo tooling)
corepack enable
pnpm install
pnpm run build
pnpm run test
pnpm run lint

# Rust side (CLI + MCP server)
cargo check --workspace
cargo test --workspace
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
```

`pnpm rust:precommit` runs the Rust fmt + clippy + check pass that the
pre-commit hook expects.

## License

Apache 2.0 — see [LICENSE](LICENSE)
