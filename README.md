# @magic-ingredients/no-tickets

Ticketless project management for AI teams — TypeScript SDK.

> **CLI + MCP retired from npm.** The CLI and MCP server have moved to a native
> Rust binary (`nt`) distributed via Homebrew, Scoop, `cargo install`, and a
> direct install script. See [docs/install.md](docs/install.md) once Task 13 lands.
> This npm package now ships the SDK only.

## What is no-tickets?

no-tickets is a project management platform for teams building with AI agents. Instead of manually creating and updating tickets, your dev tools push progress to a dashboard automatically.

- **Developers** push project state from their repos
- **PMs** see real-time progress on a hosted dashboard
- **AI agents** are first-class — their work shows up alongside human work

## SDK

For programmatic use (e.g., from tiny-brain or custom tooling):

```typescript
import { computeState, computeDiff, parseFrontmatter } from '@magic-ingredients/no-tickets/sdk';
import type { FeatureState, Phase, StateSnapshot } from '@magic-ingredients/no-tickets/types';
```

## The .notickets/ Format

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

Each file uses YAML frontmatter + markdown. See [SPEC.md](SPEC.md) for the full format specification.

## Works with tiny-brain

[tiny-brain](https://github.com/magic-ingredients/tiny-brain) is an open-source Claude Code plugin that adds TDD enforcement, quality analysis, and adversarial code review. When used with no-tickets, it also pushes telemetry data (model usage, phase compliance, quality scores) that powers the engineering dashboard.

tiny-brain is optional — no-tickets works with any tool.

## Contributing

This project uses [pnpm](https://pnpm.io/) as its package manager. The correct version is enforced via [corepack](https://nodejs.org/api/corepack.html):

```bash
corepack enable
pnpm install
pnpm run build
pnpm run test
pnpm run lint
```

## Documentation

Full docs at [docs.no-tickets.com](https://docs.no-tickets.com)

## License

Apache 2.0 — see [LICENSE](LICENSE)
