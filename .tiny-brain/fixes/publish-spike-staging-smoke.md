---
id: publish-spike-staging-smoke
title: "Actually publish an event to staging — close Task 14's wiremock-only gap"
status: completed
severity: medium
reported: 2026-05-11T00:00:00.000Z
resolved: 2026-05-11T00:00:00.000Z
resolution:
  rootCause: "Task 14 of cross-platform-cli-binary was wiremock-only despite its 'end-to-end staging' title; no event ever crossed the wire, and the wiremock contract was inferred from the TS reference rather than verified against the real server."
  fix:
    - "Built release binary at 09a1470b, ran nt publish ai.task.completed.v1 against api-staging.no-tickets.com using mystaging project's pushToken from local config."
    - "Successful publish on third attempt (after discovering real schema); event id=4 landed in mystaging's event log."
    - "Captured findings in docs/rust-spike-notes.md Task 14 section, including the real ai.task.completed.v1 schema, the working payload, and the four contract divergences from wiremock fixtures (one High, one Medium, two Low)."
  filesModified:
    - "docs/rust-spike-notes.md"
archived: true
---

# Fix: Actually publish an event to staging

## Issue Summary

**Reported:** 2026-05-11
**Severity:** medium

Task 14 of `cross-platform-cli-binary` ("Publish spike — single event to staging end-to-end") was marked completed at commit `4844b43`, but in practice the spike was **wiremock-only**. The task spec explicitly says "the wiremock suite, no real network for unit tests" (`.tiny-brain/fixes/cross-platform-cli-binary.md:610`), and every test in `crates/nt-cli/tests/publish.rs` (11 cases) hits a local wiremock instance, not real staging.

The user noticed: no event from the Rust binary has actually arrived in nt staging. Confirmed — none was sent.

What Task 14 *did* validate (against wiremock):
- reqwest + rustls TLS toolchain compiles + links + runs.
- Bearer header injection works.
- JSON request/response round-trips.
- Status-code → exit-code mapping is correct for 401 / 403 / 422 / 5xx.
- Wire-body field order matches the TS reference's emission order.

What Task 14 did NOT validate:
- **The wire contract is the wire contract that `api.no-tickets.com` actually speaks.** Wiremock asserts a self-defined shape — inferred from the TS reference, not verified against the real server. If the inferred contract diverged from reality, every test would still pass.
- **TLS chain validation against a real cert.** rustls + webpki-roots could in theory fail to recognise the real CA chain; locally we always trusted wiremock's self-signed cert (or skipped TLS).
- **End-to-end auth flow with a real push token.** No real-token round-trip has happened.
- **`docs/rust-spike-notes.md` Task 14 findings section.** The task spec required appending findings here (`cross-platform-cli-binary.md:608`); `grep "Task 14" docs/rust-spike-notes.md` returns nothing.

The architectural-debt refactor (`nt-cli-thin-edge-refactor`) just landed — it touched the transport path. Smoking against staging now also validates the refactor didn't silently break the wire path.

## Root Cause Analysis

**Task scope mis-named.** The Task 14 title and "Acceptance" line claim "end-to-end staging publish", but the implementation deliberately scoped to wiremock. There's no contradiction in the task body — the "no real network for unit tests" clause is unambiguous — but the task was marked completed without anyone *also* running the manual smoke against staging. The gap fell between "the tests pass" and "the spike's stated goal is met".

**No procedure on file.** Even if someone had wanted to smoke staging, there's no documented invocation: which env vars to set, which event type id is safe to use, which project is the no-tickets team's own staging project, where to verify arrival.

## Fix Approach

Two small, sequential pieces:

1. **Run the smoke.** Build the current binary, set staging URLs + a real push token, publish a single innocuous event with a known-schema type, capture exit code + stdout + (server-side) confirmation that the event landed.

2. **Capture the findings.** Append a "Task 14 — staging smoke findings" section to `docs/rust-spike-notes.md` recording: the exact invocation used (sans token), the observed wire response, any contract divergences from what wiremock asserted, and any TLS / auth surprises. If the smoke reveals divergences, file follow-up fixes per finding.

The smoke must be run by the human (real token, real shared system, not autonomous-agent territory). This fix doc owns the invocation + the doc update.

## Smoke Procedure

**Reproducing this smoke (post-completion reference).** The token is in `~/.notickets/config.json` under `projects.<name>.pushToken` (not in `~/.notickets/credentials` — that's the session token, which is a separate concept and isn't used by the Rust binary's publish path). The Rust binary doesn't yet read `projects.*.pushToken` directly (ADR-0002 reshapes this as `nt token add`), so the smoke threads the token via `NO_TICKETS_TOKEN`.

Fresh build first to avoid staleness:

```bash
~/.cargo/bin/cargo build --release --manifest-path crates/nt-cli/Cargo.toml
```

Then (project name `mystaging` here; substitute as appropriate):

```bash
NO_TICKETS_TOKEN=$(jq -r '.projects.mystaging.pushToken' ~/.notickets/config.json) \
  ./target/release/nt --profile staging publish \
  --type ai.task.completed.v1 \
  --data '{"taskId":"rust-spike-smoke-001","sessionId":"rust-spike-smoke-session-001","startedAt":"2026-05-11T20:30:00.000Z","completedAt":"2026-05-11T20:30:01.000Z","outcome":"success","callCount":1,"durationMs":1000}' \
  --project mystaging
```

The payload shape above is the actual `ai.task.completed.v1` schema (discovered by 422 trial-and-error in the original smoke — see `docs/rust-spike-notes.md` Task 14 section for the schema table and verbatim error responses).

Expected on success: stdout prints `{"deduped":0,"ids":["<id>"],"ingested":1}` (alphabetical field order — not what the wiremock fixture suggests), exit 0.

Then verify in the staging dashboard that the event with the returned `id` landed with the expected payload.

## Test Plan

### 🔒 Regression Tests (must pass unchanged)

| File | Cases | Status |
|------|-------|--------|
| `crates/nt-cli/tests/publish.rs` | all 11 wiremock cases | ✅ unaffected (doc-only fix; no code changed) |
| `crates/nt-cli/tests/status.rs` | all 31 cases | ✅ unaffected (doc-only fix; no code changed) |

This fix is doc-only — no production code changed. The wiremock tests still pass; their accuracy at the schema level is a known gap (finding #1) addressed by `cross-platform-cli-binary` Task 3a.

### 🆕 New Tests

None for this fix directly. The smoke is a manual verification step; its purpose is to validate the *test plan we already have* against reality. If the smoke uncovers wire-shape divergence, the follow-up fix(es) add wiremock cases pinning the *real* shape.

## Tasks

### 1. Run the staging smoke and capture the outcome
status: completed
commitSha: pending

End-to-end task: build the current release binary, run the documented smoke invocation (token from local credentials, project supplied per invocation), verify the event arrives, document the exact stdout / stderr observed.

**Files to modify:**
- `docs/rust-spike-notes.md` — append a "Task 14 — staging smoke findings" section with: date, binary commit SHA, invocation (sans token), observed stdout, observed stderr (if any), exit code, server-side confirmation method, dashboard event id, contract divergences observed (if any).

### 2. File follow-up fixes for any contract divergences
status: superseded
commitSha: null

Superseded by `cross-platform-cli-binary` Task 3a (build.rs-fetched schema bundle from GH releases). Once Task 3a lands, wiremock fixture payloads should be regenerated from the actual schemas — or, more pragmatically, tests should switch to a transport-only synthetic event-type id (e.g. `meta.test.payload.v1`) so they assert transport correctness without coupling to any one type's strict schema. **Until Task 3a lands, the High finding stands: `tests/publish.rs` payload fixtures for `ai.task.completed.v1` are wrong against the real schema.** Wiremock doesn't enforce schemas so tests pass — this is a fidelity gap, not a code bug.

The two Low findings (response field order, event id format) require no action. The Medium finding (working payload documented) is captured in `docs/rust-spike-notes.md` and in this fix doc's Smoke Procedure section.

**Files to modify:**
- Per-divergence: new fix docs under `.tiny-brain/fixes/`.
- Per-divergence: `crates/nt-cli/tests/publish.rs` and possibly `crates/nt-cli/src/commands/publish.rs` or `src/transport.rs`.
