# Cookbook: actor resolution for `no-tickets publish`

Worked examples for the common harness shapes. The full reference for
flags + the precedence chain lives in
[`../cli-reference.md`](../cli-reference.md); this doc shows how each
shape *uses* the surface.

Scenarios:
- [Single agent harness boot](#single-agent-harness-boot)
- [Multi-agent host (different sessions per agent)](#multi-agent-host)
- [Human CLI default](#human-cli-default)
- [Deliberately-unattributed publish](#deliberately-unattributed-publish)
- [CI bot / cron / migration (`agent` with no LLM context)](#ci-bot--cron--migration)
- [Per-call enrichment with a session-declared identity](#per-call-enrichment-with-a-session-declared-identity)
- [Disabling the credentials fallback on a CI box](#disabling-the-credentials-fallback-on-a-ci-box)

---

## Single agent harness boot

The common case: a Claude / Codex / tiny-brain harness wants every
event it produces in a shell to be attributable.

```bash
# Once at harness boot:
no-tickets session start \
  --agent claude \
  --model claude-opus-4-7 \
  --provider anthropic \
  --thinking-effort high \
  --session-id sess-$(uuidgen)

# Every subsequent publish auto-stamps metadata.actor:
no-tickets publish \
  --type product.feature.status_changed.v1 \
  --data '{"featureId":"f-1","fromStatus":"review","toStatus":"done"}' \
  --project demo

# On harness teardown:
no-tickets session end
```

Wire body of the publish:

```json
[{
  "type": "product.feature.status_changed.v1",
  "data": { "featureId": "f-1", "fromStatus": "review", "toStatus": "done" },
  "metadata": {
    "actor": {
      "type": "agent",
      "agentId": "claude",
      "model": "claude-opus-4-7",
      "provider": "anthropic",
      "thinkingEffort": "high",
      "sessionId": "sess-abc123"
    }
  },
  "source": { "name": "no-tickets-cli", "sdkVersion": "0.1.3", "attributes": { "project": "demo" } }
}]
```

---

## Multi-agent host

A single host running two agent harnesses concurrently (e.g.
Claude and Codex on the same machine). The default session-file path
is one-per-host; concurrent harnesses need their own files.

```bash
# Terminal 1 — Claude
NO_TICKETS_SESSION_FILE=/tmp/claude-session.json \
  no-tickets session start --agent claude --model claude-opus-4-7

NO_TICKETS_SESSION_FILE=/tmp/claude-session.json \
  no-tickets publish --type … --data … --project demo

# Terminal 2 — Codex
NO_TICKETS_SESSION_FILE=/tmp/codex-session.json \
  no-tickets session start --agent codex --model gpt-5

NO_TICKETS_SESSION_FILE=/tmp/codex-session.json \
  no-tickets publish --type … --data … --project demo
```

The `--session-file` flag is equivalent:

```bash
no-tickets publish \
  --session-file /tmp/claude-session.json \
  --type … --data … --project demo
```

Flag wins over env var when both are set; the env var alone is
sufficient when no flag is supplied.

---

## Human CLI default

After `no-tickets init`, every publish stamps a human actor
automatically — no flags, no `session start`. The credentials' email
becomes the actor's `userId`:

```bash
no-tickets init  # interactive browser login

no-tickets publish \
  --type product.feature.status_changed.v1 \
  --data '{"featureId":"f-1","fromStatus":"review","toStatus":"done"}' \
  --project demo
```

Wire actor block:

```json
{
  "actor": {
    "type": "human",
    "userId": "alice@example.com",
    "email": "alice@example.com"
  }
}
```

The `userId` collapses to `email` because the on-disk credentials file
doesn't carry a separate user-id field today. If the server later
issues a stable user-id alongside the session token, `userId` will
diverge from `email`; the schema already supports that.

For hosts where `no-tickets init` can't open a browser (CI, SSH,
sandbox), the device-code flow tracked in
[`../fixes/headless-init-device-code.md`](../fixes/headless-init-device-code.md)
lets you populate credentials non-interactively. Until that lands,
those hosts must use the agent branch (declare an explicit
`--agent <id>` via `session start` or `--agent-id` per-publish).

---

## Deliberately-unattributed publish

A caller publishing one-off events without wanting to declare any
identity. The CLI prints a one-time hint on the first such publish;
`--quiet` (or `NO_TICKETS_QUIET=1`) silences it:

```bash
# First invocation — hint fires once on stderr, marker is set:
no-tickets publish --type … --data … --project demo
#  → stderr: "Tip: this event was published without an actor. …"

# Second invocation — silent:
no-tickets publish --type … --data … --project demo

# Or suppress the hint from the start:
no-tickets publish --quiet --type … --data … --project demo
# Marker still gets written so a future un-quieted invocation stays silent.
```

The wire body has no `metadata` field at all — no
`"metadata": null`, no empty object. The omission is the contract.

---

## CI bot / cron / migration

Non-LLM systems publish as agents too, with only `agentId` set —
**no `model` field**, since there's no LLM behind them. The schema's
optional fields are simply omitted (no sentinel strings like `"n/a"`).

```bash
# In a GitHub Action workflow step:
- name: Publish ci.run.completed
  env:
    NO_TICKETS_TOKEN: ${{ secrets.NO_TICKETS_PUSH_TOKEN }}
  run: |
    no-tickets publish \
      --actor-type agent \
      --agent-id github-actions \
      --type ci.run.completed.v1 \
      --data '{"runId":"${{ github.run_id }}","outcome":"success"}' \
      --project demo
```

Wire actor block:

```json
{
  "actor": {
    "type": "agent",
    "agentId": "github-actions"
  }
}
```

Same shape for cron jobs (`--agent-id cron`), migrations
(`--agent-id migration`), or any other non-LLM publisher. If a
`system` variant ever proves useful in practice, it'll land as a v1.1
schema branch — until then, `agent` with just `agentId` is the shape.

---

## Per-call enrichment with a session-declared identity

The common LLM-publish shape: identity comes from the session;
per-call fields (token counts, latency, call id) layer per publish.

```bash
no-tickets session start --agent claude --model claude-opus-4-7

# Publish with per-call enrichment:
no-tickets publish \
  --call-id call-$(uuidgen) \
  --prompt-tokens 1234 \
  --completion-tokens 567 \
  --latency-ms 812 \
  --type ai.completion.recorded.v1 \
  --data '{"completionId":"c-1"}' \
  --project demo
```

Wire actor block:

```json
{
  "actor": {
    "type": "agent",
    "agentId": "claude",
    "model": "claude-opus-4-7",
    "callId": "call-abc",
    "promptTokens": 1234,
    "completionTokens": 567,
    "latencyMs": 812
  }
}
```

The agent identity (`agentId`, `model`) comes from the session file;
the per-call fields come from the flags. The session file itself
never stores `callId` / tokens / latency — those are publish-time
concerns.

---

## Disabling the credentials fallback on a CI box

A CI host has `no-tickets init` credentials sitting around from local
debugging, but the CI job should publish as the `github-actions`
agent — not as the developer who ran `init` locally. Pass
`--actor-type=agent` to disable the credentials fallback:

```bash
no-tickets publish \
  --actor-type agent \
  --agent-id github-actions \
  --type ci.run.completed.v1 \
  --data '{"runId":"42"}' \
  --project demo
```

With `--actor-type=agent`, the resolver skips the credentials branch
entirely. If no agent-producing branch resolves, the result is
unattributed (not "fall through to credentials"). The flag becomes a
*constraint* — "publish only as an agent, or not at all" — not a
preference.

The symmetric form is `--actor-type=human`: forces the
credentials/unattributed sub-chain even if a session file is present.
Useful when a stale session file is sitting on disk and you want
this single publish to come from the human identity.

---

## See also

- [`../cli-reference.md`](../cli-reference.md) — full flag surface
  and resolution-precedence reference
- [`event-actor-metadata` PRD](https://github.com/magic-ingredients/no-tickets-service/blob/main/docs/prd/event-actor-metadata/prd.md)
  — design rationale + non-goals (canonical home; lives in the
  `no-tickets-service` repo because most of the work + the schema
  source-of-truth live there)
- [`../fixes/event-actor-metadata-client.md`](../fixes/event-actor-metadata-client.md)
  — client-side task tracker (this repo)
