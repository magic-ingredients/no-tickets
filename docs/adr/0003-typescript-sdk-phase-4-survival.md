---
adr_number: 3
title: "TypeScript SDK Phase 4 survival: keep the npm package as a thin wrapper"
date: 2026-05-20
status: accepted
supersedes: null
superseded_by: null
tags: [architecture, sdk, distribution, phase-4]
decision_makers: [Andy Richardson]
---

# ADR-0003: TypeScript SDK Phase 4 survival

## Status

Accepted

## Context

Phase 2/3 of the cross-platform-cli-binary fix retired the npm-shipped
TypeScript CLI and MCP server. The Rust binary `no-tickets` is now the
sole canonical CLI/MCP surface. The npm package
`@magic-ingredients/no-tickets` was downgraded to ship the
`.notickets/` SDK only (markdown parsing, state computation, frontmatter
helpers) — no transport, no publish, no CLI binary.

Phase 4 of the same roadmap calls for per-language wrappers that spawn
the Rust binary in `--stream` mode for warm in-process publishing.
Python and Go wrappers are clearly net-new packages. The open question
this ADR resolves:

> **Should the TypeScript wrapper come back?**

Two phrasings of "no":

- **"Rust binary only forever"** — every language calls `execFile('no-tickets', …)`
  themselves. The npm SDK stays markdown-only. No Phase 4 TS wrapper.

- **"Phase 4 TS wrapper, but keep the SDK package separate"** — ship
  `@magic-ingredients/no-tickets-client` as a new npm package that
  spawns the binary; the existing SDK package stays markdown-only.

One phrasing of "yes":

- **"TS wrapper as the same SDK package"** — the existing
  `@magic-ingredients/no-tickets` package grows a `publish` /
  `validateEvent` export that internally spawns `no-tickets-mcp` in
  `--stream` mode. One package for both SDK + client.

## Decision

**Phase 4 reintroduces a thin TypeScript wrapper, shipped as the same
npm package** (`@magic-ingredients/no-tickets`). Same package, expanded
exports.

The wrapper is a ~50-80 LOC `execFile` / `subprocess.Popen` shim over
`no-tickets-mcp --stream`, plus typed errors and typed promises. It
joins the existing markdown SDK exports under the same import root:

```typescript
import { computeState, parseFrontmatter } from '@magic-ingredients/no-tickets/sdk';     // existing
import { publish, validateEvent } from '@magic-ingredients/no-tickets/client';          // new in Phase 4
```

## Rationale

### Why a TS wrapper at all

Three reasons:

1. **Discoverability.** TS-shop teams looking for an npm package will
   find `@magic-ingredients/no-tickets`. If that package only exposes
   markdown helpers and nothing for publishing events, they reasonably
   conclude there's no JS path and reach for HTTP themselves. A
   spawn-based client preserves the "if you `npm install` it, it has
   what you need" UX.

2. **Type safety.** TypeScript callers writing `subprocess.exec` lose
   typing on the event-type → payload-shape mapping. A wrapper can
   import the auto-generated Zod schemas (from the schemas pipeline)
   and surface `publish<T extends EventType>(type: T, data: PayloadFor<T>)`.
   Raw `exec` can't do that without re-implementing the type plumbing.

3. **Consistent with Python/Go.** Phase 4 ships those wrappers
   regardless. Skipping TS makes it the odd-one-out language, which
   inverts the "first-class for every language" framing of the fix
   doc's user journey table.

### Why the same npm package (vs. a separate one)

Two packages would split the import surface (`@magic-ingredients/no-tickets-sdk`
+ `@magic-ingredients/no-tickets-client`) and force users to install
both for the common case. The current `@magic-ingredients/no-tickets`
already has the right name, npm presence (~12k downloads/month at the
last check), and import ergonomics — extending it costs nothing and
reuses the user mental model.

The split *exports* approach (`/sdk`, `/client`) keeps the legacy
markdown helpers and the new client cleanly separated within the
package, so consumers who only want one half pay for only one half via
bundler tree-shaking.

### Why not "Rust binary only forever"

That would be the simplest option (zero npm code to maintain). It
fails the discoverability + type-safety arguments above. Also: the
binary-spawn pattern is already half-built — Phase 4 ships Python and
Go wrappers regardless, so the wrapper template (process management,
JSONL framing, error translation) exists. The marginal cost of a TS
wrapper on top is ~80 LOC; the saving from skipping it is small.

### Why not bring back the full TS CLI

That was explicitly killed in Task 12 of cross-platform-cli-binary.
Reason: the TS CLI duplicated the canonical Rust binary surface and
created two source-of-truth places for command shape, error messages,
exit codes. Replacing it with `execFile` makes the Rust binary the
sole source-of-truth and keeps the TS surface as a syntactic shim,
not a parallel implementation.

The wrapper IS spawn-glue. It is NOT a TS CLI. Functionally:

```typescript
// what the wrapper IS
async function publish<T extends EventType>(args: PublishArgs<T>): Promise<PublishResult> {
  const proc = ensureStreamProcess();          // spawn-once, reuse
  return proc.send({ type: args.type, data: args.data, project: args.project });
}

// what the wrapper is NOT
function nt(args: string[]): never;            // no CLI surface in JS
function parseFlags(...): never;               // no arg parsing in JS
function buildEnvelope(...): never;            // no transport logic in JS
```

## Consequences

### Positive

- TS teams have a one-line install + typed-promise API for publishing.
- The binary remains the single source of truth for wire protocol,
  validation, retry, error mapping — wrapper is a thin spawn shim.
- Python and Go wrappers share the same architectural template
  (spawn `no-tickets-mcp --stream`, frame JSONL, translate errors).

### Negative

- The `@magic-ingredients/no-tickets` package now has a hard runtime
  dependency on the `no-tickets-mcp` binary being on PATH. Documented
  via a clear error message ("install no-tickets-mcp first: see
  https://get.no-tickets.com") rather than a cryptic ENOENT.
- The wrapper's typed-payload story depends on the schemas-from-Zod
  codegen pipeline being mature enough to produce TS types
  per event-type. That codegen is a separate dependency (server-side);
  if it slips, the wrapper ships with looser types until it lands.

### Neutral

- Update `docs/install.md` to mention the TS wrapper in the Phase 4
  "Coming soon" section.
- Bump `@magic-ingredients/no-tickets` major when the client exports
  are added (the SDK-only contract is what users on the current
  3.x version have).

## Dependencies and ordering

Phase 4 (this ADR's scope) depends on:

- **stream-mode fix** — `no-tickets-mcp --stream` shipped (the wrapper
  has nothing to spawn-and-frame against without it)
- **schemas-from-Zod codegen** — for per-event-type typed payloads
  (without it, the wrapper still ships, just with `data: unknown`)

Ordering: stream-mode lands first, then this wrapper, then the codegen
slot-in for typed payloads. Each is independently shippable.

## Open questions deferred

- Should the wrapper auto-install `no-tickets-mcp` via npm postinstall?
  Strong "no" instinct (postinstall scripts that download binaries are
  a known security smell), but document the alternative ("we throw a
  clear error and link to the install command") explicitly when the
  wrapper ships.
- Browser support? No. The wrapper spawns a subprocess; it's Node-only.
  Browser-side use cases hit the server's HTTPS API directly.
