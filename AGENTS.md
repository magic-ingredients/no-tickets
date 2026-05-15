<!-- agents:tiny-brain:start -->
<!-- content-hash:277f4a39920ae799 -->
# @magic-ingredients/no-tickets

SDK for no-tickets — ticketless project management for AI teams

Ticketless project management for AI teams — TypeScript SDK.

## Environment Setup

**Prerequisites:**
- Node.js 25 (from `.node-version`)

**Quick start:**
```bash
pnpm install
```

## Development

**Start dev server:**
```bash
tsc --watch
```

**Build:**
```bash
tsc -p tsconfig.build.json
```

## Testing

**Framework:** vitest

**Run tests:**
```bash
pnpm test
```

**Coverage:**
```bash
vitest run --coverage
```

**Test file patterns:** `.test.ts`

**Run a single test file:**
```bash
vitest run path/to/file.test.ts
```

## Linting & Code Style

**Linter:** eslint

**Run lint:**
```bash
pnpm lint
```

**Plugins:** typescript

## Coding Conventions

- TypeScript strict mode enabled
- Test files follow pattern: `*.test.ts`

## Commit Guidelines

Before committing, ensure:
- Lint passes (`pnpm lint`)
- Tests pass (`pnpm test`)

**CI:** GitHub Actions runs on pull requests.

## Tech Stack

See `.tiny-brain/analysis.json` for the full detected tech stack.

## Project Structure

**Source directories:** `src/`
<!-- agents:tiny-brain:end -->