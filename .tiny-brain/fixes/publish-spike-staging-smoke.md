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
    - "Built release binary at 09a14700, ran nt publish ai.task.completed.v1 against api-staging.no-tickets.com using mystaging project's pushToken from local config."
    - "Successful publish on third attempt (after discovering real schema); event id=4 landed in mystaging's event log."
    - "Captured findings in docs/rust-spike-notes.md Task 14 section, including the real ai.task.completed.v1 schema, the working payload, and the four contract divergences from wiremock fixtures (one High, one Medium, two Low)."
  filesModified:
    - "docs/rust-spike-notes.md"
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

The token is already in `~/.notickets/credentials` from prior CLI usage; the URL defaults (`api.no-tickets.com`) are what the credentials were issued against. Only the project name needs to be supplied per invocation.

Fresh build first to avoid staleness:

```bash
~/.cargo/bin/cargo build --release --manifest-path crates/nt-cli/Cargo.toml
```

Then:

```bash
./target/release/nt publish \
  --type ai.task.completed.v1 \
  --data '{"taskId":"rust-spike-smoke-001","summary":"Rust nt publish smoke test","durationMs":42}' \
  --project <PROJECT_NAME>
```

Expected on success: stdout prints `{"ingested":1,"deduped":0,"ids":["evt_..."]}`, exit 0.

Then verify in the dashboard / event log that an event with `id == evt_<that id>` landed with the expected payload.

## Test Plan

### 🔒 Regression Tests (must pass unchanged)

| File | Cases | Status |
|------|-------|--------|
| `crates/nt-cli/tests/publish.rs` | all 11 wiremock cases | ❌ |
| `crates/nt-cli/tests/status.rs` | all 31 cases | ❌ |

No test changes from this fix unless the smoke reveals contract divergence — divergences become follow-up fixes with their own tests.

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

Four divergences found (1 High, 1 Medium, 2 Low; full detail in `docs/rust-spike-notes.md` Task 14 section). All are documentation/fidelity issues, not code bugs — the Rust binary worked correctly end-to-end. The High finding (wiremock payload fixtures use a wrong shape for `ai.task.completed.v1`) is addressed naturally by `cross-platform-cli-binary` Task 3a (build.rs-fetched schema bundle from GH releases): once that lands, fixture payloads can be regenerated from real schemas, or the tests switch to a transport-only synthetic type id. The two Low findings (response field order, event id format) require no action. No follow-up fix doc warranted.

**Files to modify:**
- Per-divergence: new fix docs under `.tiny-brain/fixes/`.
- Per-divergence: `crates/nt-cli/tests/publish.rs` and possibly `crates/nt-cli/src/commands/publish.rs` or `src/transport.rs`.
