# `no-tickets` CLI reference

Public, stable contract for the `no-tickets` binary's command surface.
Companion to [`binary-error-contract.md`](./binary-error-contract.md)
(which pins exit codes + stderr shapes).

Sections:
- [`no-tickets publish`](#no-tickets-publish) — single-event publish
  with optional actor attribution
- [`no-tickets session`](#no-tickets-session) — agent-harness identity
  lifecycle
- [Actor resolution precedence](#actor-resolution-precedence) — how
  `metadata.actor` gets stamped on the wire
- [First-publish hint](#first-publish-hint) — one-time reminder for
  unattributed publishes
- [Environment variables](#environment-variables) — `NO_TICKETS_*`
  surface

---

## `no-tickets publish`

Send one event to `/v1/events`. Two modes:

- **Single event** — `--type` + `--data` (a JSON payload string).
- **Batch** — `--file <path>` (or `-` for stdin) — JSONL, one event
  per line.

```bash
no-tickets publish \
  --type product.feature.status_changed.v1 \
  --data '{"featureId":"f-1","fromStatus":"review","toStatus":"done"}' \
  --project demo
```

### Required flags

| Flag | Description |
|---|---|
| `--type <id>` | Event type id (e.g. `ai.task.completed.v1`). Required in single-event mode. |
| `--data <json>` | Event payload as a JSON string. Required in single-event mode. |
| `--project <key>` | Local project key. Looks up the push token registered via `no-tickets token add`. The server resolves the actual project from the token; the flag value is not sent on the wire. |

### Optional event-level flags

| Flag | Wire field | Notes |
|---|---|---|
| `--source-name <name>` | `source.name` | Overrides the default `no-tickets-cli`. |
| `--source-attribute KEY=VALUE` | `source.attributes.KEY` | May repeat; last value wins on duplicate keys. |
| `--parent <event-id>` | `parentEventId` | Single-event mode only. |
| `--trace <id>` | `traceId` | Single-event mode only. |
| `--dedupe-key <key>` | `dedupeKey` | Idempotency key. Single-event mode only. |
| `--file <path>` | (batch JSONL) | Mutually exclusive with `--type`/`--data`. |

### Actor-attribution flags (opt-in)

When supplied, these flags populate `metadata.actor` on the wire. When
**none** are supplied AND no session is declared AND no credentials are
present, `metadata` is omitted entirely from the envelope — the
unattributed publish is permanently valid. See [Actor resolution
precedence](#actor-resolution-precedence) below.

| Flag | Wire field on `metadata.actor` | Notes |
|---|---|---|
| `--actor-type <human\|agent>` | `actor.type` | Constrains which resolution branches apply. See precedence below. |
| `--agent-id <id>` | `actor.agentId` | Triggers the flag-driven agent branch. The only required identity field for `agent` actors. |
| `--model <id>` | `actor.model` | LLM model identifier. Omitted for non-LLM systems. |
| `--provider <id>` | `actor.provider` | LLM provider identifier. |
| `--thinking-effort <low\|medium\|high>` | `actor.thinkingEffort` | Three-level enum. |
| `--session-id <id>` | `actor.sessionId` | Opaque session id grouping events from one harness run. |
| `--call-id <id>` | `actor.callId` | **Per-call** — stamped on this single publish only. |
| `--prompt-tokens <N>` | `actor.promptTokens` | Per-call. |
| `--completion-tokens <N>` | `actor.completionTokens` | Per-call. |
| `--latency-ms <N>` | `actor.latencyMs` | Per-call. |
| `--session-file <path>` | (path) | Alternate session-file path. Equivalent to `NO_TICKETS_SESSION_FILE`. |
| `--quiet` | (none) | Suppresses the first-publish hint on stderr. Does NOT suppress the `state.json` marker write. |

#### Per-call vs. session-context fields

- **Identity:** `--agent-id` for agents; the credentials' `email`
  becomes `userId` for humans.
- **Session-context:** `--model`, `--provider`, `--thinking-effort`,
  `--session-id` — typically set once per harness run via
  `no-tickets session start` and inherited by every subsequent
  publish.
- **Per-call enrichment:** `--call-id`, `--prompt-tokens`,
  `--completion-tokens`, `--latency-ms` — never stored on the session
  file; layered onto the resolved agent actor at publish time.

Per-call enrichment is silently dropped for the human variant — the
canonical `humanActorSchema` carries only `userId` and optional
`email`. That's a documented limitation, not an error.

#### Non-goals for `metadata.actor`

The actor block is identity + cost only. It does **not** carry:

- Prompt content
- Completion text
- Tool-call arguments
- Reasoning traces

Those live in `ai.*` event `data` when they are themselves the fact
being recorded. `metadata.actor` is the *who* + *cost*, not the *what*.

The CLI also **never** infers actor identity from environment
variables like `CLAUDECODE`, `GITHUB_ACTIONS`, or `CI`. Attribution
is what the caller explicitly declared, never what the environment
implies — auto-attribution would make the audit trail agree with
anything a process claims, defeating its purpose.

---

## `no-tickets session`

Manages the agent-harness identity carried in `<config-dir>/active-session.json`.
A subsequent `no-tickets publish` reads that file and stamps the
declared identity onto every envelope's `metadata.actor` — no
per-publish flag repetition.

`<config-dir>` resolves per [ADR-0002](./adr/) — platform-native via
`directories::ProjectDirs::from("com", "magic-ingredients", "no-tickets")`,
overridable via `NO_TICKETS_HOME=<dir>` (which yields
`<dir>/.notickets/`).

### `no-tickets session start`

```bash
no-tickets session start \
  --agent claude \
  --model claude-opus-4-7 \
  --provider anthropic \
  --thinking-effort high \
  --session-id sess-$(uuidgen)
```

Writes `<config-dir>/active-session.json` atomically (temp + rename).

| Flag | Required | Notes |
|---|---|---|
| `--agent <id>` | yes | Agent identifier (`claude`, `codex`, `tiny-brain`, `github-actions`, …). Only mandatory flag. |
| `--model <id>` | no | Omit for non-LLM systems (CI bots, cron, migrations). |
| `--provider <id>` | no | |
| `--thinking-effort <low\|medium\|high>` | no | |
| `--session-id <id>` | no | |
| `--max-age-hours <N>` | no | Default `24`. Hard cap `168` (7 days). Range enforced by clap. |

Omitted flags are **omitted from the actor block** — never stored as
sentinel strings like `"n/a"`.

### `no-tickets session show`

Prints the active session as JSON, or `{"active":false}` when no
session is set:

```json
{
  "active": true,
  "actor": {
    "type": "agent",
    "agentId": "claude",
    "model": "claude-opus-4-7",
    "provider": "anthropic",
    "thinkingEffort": "high",
    "sessionId": "sess-abc123"
  },
  "startedAt": "2026-05-21T10:00:00.123Z",
  "pid": 12345,
  "maxAgeHours": 24,
  "expired": false
}
```

`expired` becomes `true` when `now > startedAt + maxAgeHours`. The
publish-time resolver treats expired sessions as if absent — it falls
through to credentials/unattributed.

### `no-tickets session end`

Deletes `<config-dir>/active-session.json` (no-op when absent) **and**
clears `firstPublishHintShown` from `<config-dir>/state.json` (no-op
when absent — does not create the file just to write a `false` flag).
Always exits 0.

Clearing the hint marker is intentional: after `session end`, the
next unattributed publish re-fires the one-time hint, so an operator
who switched harness shapes hears about attribution again at the
appropriate moment.

---

## Actor resolution precedence

`no-tickets publish` walks the precedence chain exactly once per
invocation. The first branch to produce an actor wins; per-call
enrichment flags layer on top of the resolved identity.

```
1. --agent-id flag present                                → flags
   (constrained out by --actor-type=human)

2. --session-file <path> OR NO_TICKETS_SESSION_FILE env   → session-env
   (constrained out by --actor-type=human)

3. <config-dir>/active-session.json present + fresh       → active-session
   (constrained out by --actor-type=human)

4. session credentials present (no-tickets init was run)  → credentials
   (constrained out by --actor-type=agent)

5. otherwise                                              → unattributed
                                                            (one-time hint
                                                             on stderr —
                                                             see below)
```

`--actor-type` is a **constraint** the caller uses to opt out of the
wrong identity surface — a CI runner with stray `init` credentials
that wants to publish under its `agent` label passes
`--actor-type=agent`; the credentials branch is then skipped.

There is **no error branch** in this chain. Publish always succeeds
when the envelope itself is valid; missing actor is a permanent valid
outcome.

Per-call enrichment flags (`--call-id`, `--prompt-tokens`,
`--completion-tokens`, `--latency-ms`) overlay on whichever branch
won, *except* on the credentials branch — humans don't carry per-call
fields in the canonical schema.

`--stream` (tracked in [`fixes/stream-mode.md`](./fixes/stream-mode.md))
will reuse this same resolver at process startup, caching the result
for the stream's lifetime. The hint, when it fires under `--stream`,
prints before any event JSON flows.

---

## First-publish hint

When publish resolves to no actor (branch 5) **and** the marker file
`<config-dir>/state.json` does not have `firstPublishHintShown: true`,
the CLI prints a one-time hint to stderr:

```
Tip: this event was published without an actor. Events without actor metadata
are not attributable in the UI or activity feed.

To attach actor info to every publish in this shell, run once:
  no-tickets session start --agent <your-agent-id>

(This hint shows only once. `no-tickets session end` clears it so it can fire
again next time you're in the no-session state.)
```

After printing, the CLI atomically updates `state.json` to set
`firstPublishHintShown: true`. The hint is informational only — exit
code stays 0 and the event lands normally.

Subsequent unattributed publishes from the same `<config-dir>` stay
silent until `no-tickets session end` (or manual edit of `state.json`)
clears the marker.

### Suppressing the hint

- `--quiet` — suppresses the stderr text but **still sets the marker**
  so the env-var doesn't have to stay set forever.
- `NO_TICKETS_QUIET=<any-non-empty>` — equivalent to `--quiet` for the
  current invocation. Same marker-still-written semantics.

The hint **never** fires on a failed publish: failures emit only the
single-line JSON structured-error envelope on stderr (per
[`binary-error-contract.md`](./binary-error-contract.md)), and the
marker isn't written. A future successful unattributed publish on the
same `<config-dir>` will see the marker unset and fire the hint then.

---

## Environment variables

| Var | Purpose |
|---|---|
| `NO_TICKETS_HOME` | Overrides `<config-dir>` to `<NO_TICKETS_HOME>/.notickets/`. Per ADR-0002. |
| `NO_TICKETS_API_URL` + `NO_TICKETS_AUTH_URL` | Override URL pair for non-prod environments. Both must be set together. |
| `NO_TICKETS_ENV` | Selects a known env (e.g. `staging`). Mutually exclusive with the explicit URL pair. |
| `NO_TICKETS_TOKEN` | Escape hatch — bypasses the project/token registry and uses this push token verbatim. CI use case. |
| `NO_TICKETS_SESSION_FILE` | Alternate path for the session file (branch 2 of the resolver chain). |
| `NO_TICKETS_QUIET` | Non-empty value suppresses the first-publish hint. Marker still written. |
| `NO_TICKETS_INCLUDE_MACHINE` | When `=1`, adds a machine-fingerprint hash to `source.attributes.machine`. Off by default. |

`NT_RETRY_BASE_DELAY_MS` is a test-only knob; not part of the public
contract.

---

## See also

- [`binary-error-contract.md`](./binary-error-contract.md) — exit
  codes and structured stderr shapes
- [`cookbook/actor-resolution.md`](./cookbook/actor-resolution.md) —
  worked examples per harness shape
- [`fixes/stream-mode.md`](./fixes/stream-mode.md) — protocol for the
  long-running `--stream` mode (reuses this resolver at startup)
- [`fixes/headless-init-device-code.md`](./fixes/headless-init-device-code.md) —
  device-code flow for hosts where `no-tickets init` can't open a
  browser (enables the human-actor branch on CI / SSH / sandbox hosts)
