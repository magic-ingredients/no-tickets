# Rust spike notes — `cross-platform-cli-binary` Task 1

Findings from porting `nt status` to Rust as a toolchain spike before
committing to the full CLI rewrite (Tasks 4–5 in the fix doc).

**Commits in this spike:** `38defa7` → `d6814eb` → `3b8e15c` → `9cdeb2d` →
`3578250` → `80f18ea`. Six commits, 30 passing tests, single command
end-to-end.

## Toolchain choices that landed

All from the crate audit in the fix doc; the spike confirms each one
holds up at parity work.

| Concern | Crate | Notes from the spike |
|---|---|---|
| CLI parsing | `clap` (derive) | `#[arg(long, global = true)]` makes a flag work both before and after the subcommand — needed for TS argv parity (`nt --profile X status` ≡ `nt status --profile X`). Forces the test to pin both forms; both pass. |
| JSON in/out | `serde` + `serde_json` | Field order on the wire is governed by struct-field declaration order; no `preserve_order` feature needed for output. For the `Available: a, b` hint where on-disk profile insertion order matters, `serde_json::Map` is BTreeMap-backed by default — see "Surprises" below. |
| JSON map preserving insertion order | `indexmap` (with `serde` feature) | Required for any config map where on-disk key order is user-visible. Don't use `BTreeMap` for that case (alphabetises) and don't use `HashMap` (non-deterministic). |
| ISO 8601 expiry parsing | `time` (parsing feature only) | Drop `formatting` and `macros` features — not needed for parse-only consumers. `OffsetDateTime::parse(s, &Iso8601::DEFAULT)` handles `YYYY-MM-DDTHH:MM:SS.sssZ` from JS `Date.toISOString()` cleanly. |
| URL validation | `url` | `Url::parse` + scheme check + non-empty-host requirement is the right replacement for a `starts_with("https://")` prefix check. Will be transitively present via `reqwest` later anyway. |
| Test harness | `assert_cmd` + `predicates` + `tempfile` | Standard. `assert_cmd::Command::env_remove` plus `tempfile::tempdir()` give complete env-and-fs isolation. Use `predicate::str::contains(...).not()` for negative assertions. |
| Mutation testing | `cargo-mutants` | **Not Stryker** — Stryker (this repo's `stryker.config.mjs`) is wired for the TS package via `@stryker-mutator/vitest-runner` and doesn't support Rust. `cargo-mutants` is the Rust standard; `cargo install cargo-mutants` then `cargo mutants -f <files>`. Spike result: 19 killed / 1 equivalent (genuine) / 4 unviable on the changed files. |

## Module layout

```
crates/nt-cli/src/
├── main.rs          clap entry, dispatches to subcommands
├── home.rs          NO_TICKETS_HOME > HOME (Unix) / USERPROFILE (Windows)
├── credentials.rs   load ~/.notickets/credentials (shape + expiry validated)
├── auth.rs          env-token > credentials-file, push/session/unknown
├── urls.rs          --profile > env-vars (pair-validated) > defaults
└── status.rs        status command: URLs first, then auth, JSON to stdout
```

Responsibilities are cleanly separated — each module reflects one TS source
file from `src/sdk/`. The adversarial reviewer flagged in 9cdeb2d that
`urls.rs` carries the config-file I/O, which will need to be shared with
future commands (`init`, `project`, `connect`). **Action for Task 4:**
extract `config.rs` covering config-file read/parse/persist before adding
the second subcommand that touches it.

## Surprises

1. **BTreeMap silently re-orders profiles.** Initial impl used
   `BTreeMap<String, ProfileConfig>` for the `profiles` field of
   `config.json`. The "Available: …" hint emerged alphabetised. TS uses
   `Object.keys()` which is insertion order. Fixed with `IndexMap`
   (commit `3578250`); test pinned (commit `80f18ea`). Lesson: any
   on-disk map whose ordering is user-visible needs `IndexMap`.

2. **`fs::read_to_string` IO errors are not "invalid JSON".** Brief
   intermediate state mapped `io::ErrorKind::InvalidData` to a JSON
   parse error variant. Wrong: `InvalidData` from `read_to_string`
   means non-UTF-8 bytes, not malformed JSON. All `read_to_string`
   errors → "could not be read" is the right call; `serde_json::from_str`
   handles actual parse failures separately. Fixed in `80f18ea`.

3. **`starts_with("https://")` is not URL validation.** Accepts
   `https://` (empty host), `http:// nope` (space), embedded newlines,
   etc. Use `url::Url::parse` plus a `host_str().is_some_and(non_empty)`
   check. Pinned by `status_profile_https_without_host_is_invalid`.

4. **`println!` panics on broken pipe.** Default behaviour when
   stdout closes (e.g. `nt status | head -n 0`). Use `writeln!` against
   locked stdout with explicit `ErrorKind::BrokenPipe` handling.

5. **`home::home_dir()` failure must be a real error, not a panic.**
   The initial draft used `.expect("home dir resolvable")` for the URL
   path; with `NO_TICKETS_HOME`, `HOME`, and `USERPROFILE` all unset
   the binary would crash with a Rust stacktrace. Surface as
   `UrlError::HomeUnresolvable`. Pinned by
   `status_profile_with_no_home_resolvable_errors_gracefully`.

6. **Adversarial review fires after every `feat:`/`refactor:` commit
   on a tracked task.** Plan for ~2–3 iterations per task: write
   tests, implement, review, refactor, review again. Budget for it.

## Deliberate TS divergence

**Unparseable `expiresAt` is treated as not-authenticated.**

The TS impl computes `new Date(parsed.expiresAt).getTime()` which returns
`NaN` for unparseable strings; `NaN <= Date.now()` is `false` in JS, so
TS accidentally accepts the credential. The Rust port rejects it. This
is a designed behaviour, not parity — pinned by
`status_credentials_unparseable_expires_at_is_not_authenticated`. Worth
back-porting to TS during the broader rewrite.

## Performance

Cold cargo-build (debug): ~7.6 s on M1 Max, 71 dependencies. With release
optimisations the binary is ~3.5 MB stripped. Cold-start latency under
`cargo run` is dominated by load time; the actual binary cold-starts
in ~5 ms — well inside the fix doc's sub-50 ms target.

## Recommendations for Task 4 (full CLI port)

1. **Extract `config.rs`** before adding the second subcommand that
   touches `~/.notickets/config.json`. Currently lives in `urls.rs`.

2. **Add a `ProfileFileUnreadable` test** with a real fs failure
   (directory at the config path, or file with no read permission)
   to pin the IO-vs-JSON split. The IO-classification fix in `80f18ea`
   is correct by inspection but unpinned.

3. **Backport the unparseable-`expiresAt` fix to TS** as a separate
   commit on the same fix — defensible parity bug fix.

4. **Audit every `.expect()` / `.unwrap()`** before reaching
   user-facing paths. Each one is a potential Rust-stacktrace crash;
   convert to typed errors with `user_message()` per the existing
   `UrlError` pattern.

5. **Plan for `cargo-mutants` budget.** Mutation review is mandatory
   per the fix pipeline. Each command (`init`, `publish`, `project`,
   `validate`, `connect`, `token`) will need ~30–60 s of mutation
   runtime. Roughly 1–2 minutes per task.

6. **Consider extracting a `cli-test-support` dev-dep crate** when
   the second subcommand's test file starts duplicating the
   `isolate()` / `write_credentials()` / `write_config()` helpers.

7. **For `--profile` resolution alongside other commands**, the
   global-arg pattern (clap `global = true`) holds up. Don't make
   `--profile` per-subcommand or argv-position parity breaks.

## What the spike does NOT validate

- **MCP-server-side via `rmcp`.** Task 2 spike covers that — separate
  exercise.
- **HTTP transport via `reqwest` + `rustls`.** `nt status` makes no
  network call; the spike validates auth/URL plumbing only. Will be
  exercised by Task 4 commands (`publish`, `validate`).
- **Cross-compile.** Spike built for the host (`aarch64-apple-darwin`)
  only. `cargo-zigbuild` / `cargo-dist` work belongs to Task 6.
- **`build.rs` JSON Schema bundle integration.** Task 3 separately.

## Verdict

Toolchain is solid. No toolchain-level surprises that change the
plan. Proceed with Task 2 (`rmcp` spike) in parallel with Task 4
(full CLI port).

---

# Task 14 — staging smoke findings

**Date:** 2026-05-11
**Binary commit:** `09a1470b` (from `nt-cli-thin-edge-refactor`)
**Tracked by fix:** `publish-spike-staging-smoke`

Task 14's wiremock-only test plan was completed at `4844b43` but no actual staging publish was performed at that time. This section captures the first real end-to-end staging smoke, run after the `nt-cli-thin-edge-refactor` work landed.

## Invocation

```bash
NO_TICKETS_TOKEN=$(jq -r '.projects.mystaging.pushToken' ~/.notickets/config.json) \
  ./target/release/nt --profile staging publish \
  --type ai.task.completed.v1 \
  --data '<see "Working payload" below>' \
  --project mystaging
```

- Token sourced from the local `mystaging` project entry in `~/.notickets/config.json` (the Rust binary doesn't yet read `projects.*.pushToken` directly — Task 5 of `cross-platform-cli-binary` will land that lookup; ADR-0002 reshapes it as `nt token add`).
- `--profile staging` resolves to `https://api-staging.no-tickets.com` via the existing TS-compatible profile loader in `urls.rs`.
- `--project mystaging` flows through to `source.attributes.project` on the wire (informational; routing was driven by the bearer token).

## Result

**Successful publish on the third attempt.** First two attempts returned 422 with the verbatim server responses below; third attempt (with the correctly-shaped payload from "Working payload" further down) succeeded.

### Attempt 1 — payload with `summary` field, missing required fields

Sent: `{"taskId":"rust-spike-smoke-001","summary":"Rust nt publish smoke test","durationMs":42}`

Server response (HTTP 422), exit 1, stderr:

```
server returned 422: {"error":"Validation failed","errors":[{"batchIndex":0,"issues":[{"expected":"string","code":"invalid_type","path":["sessionId"],"message":"Invalid input: expected string, received undefined"},{"expected":"string","code":"invalid_type","path":["startedAt"],"message":"Invalid input: expected string, received undefined"},{"expected":"string","code":"invalid_type","path":["completedAt"],"message":"Invalid input: expected string, received undefined"},{"code":"invalid_value","values":["success","partial","failed","abandoned"],"path":["outcome"],"message":"Invalid option: expected one of \"success\"|\"partial\"|\"failed\"|\"abandoned\""},{"expected":"number","code":"invalid_type","path":["callCount"],"message":"Invalid input: expected number, received undefined"},{"code":"unrecognized_keys","keys":["summary"],"path":[],"message":"Unrecognized key: \"summary\""}]}]}
```

### Attempt 2 — added all required string + enum fields, omitted `durationMs`

Server response (HTTP 422), exit 1, stderr:

```
server returned 422: {"error":"Validation failed","errors":[{"batchIndex":0,"issues":[{"expected":"number","code":"invalid_type","path":["durationMs"],"message":"Invalid input: expected number, received undefined"}]}]}
```

### Attempt 3 — added `durationMs`

Server response (HTTP 200), exit 0, stdout:

```json
{"deduped":0,"ids":["4"],"ingested":1}
```

Confirmed by server response — event with `id=4` landed in `mystaging`'s event log.

### Error envelope shape (incidental finding)

The 422 responses follow a consistent shape worth noting for whoever builds the structured-error contract (Task 4a of `cross-platform-cli-binary`):

```
{
  "error": "Validation failed",
  "errors": [
    {
      "batchIndex": <int>,
      "issues": [
        { "code": "<zod-code>", "path": [<string|int>...], "message": "<text>", "expected"?: "<type>", "values"?: [...], "keys"?: [...] }
      ]
    }
  ]
}
```

`batchIndex` is per-envelope (currently always 0 since the wire body is single-element); `issues` is a flat array of Zod-style validation problems. Useful for the structured-error contract to surface per-field validation errors with exit-code metadata.

## What was validated end-to-end (previously only wiremock-asserted)

| Concern | Result |
|---|---|
| TLS chain validation against real cert | ✅ rustls + webpki-roots accepts `*.no-tickets.com` cert chain. No `Network` errors from reqwest. |
| Bearer auth with a real `nt_push_*` token | ✅ Server accepted the credential; no 401. Routed to the validation layer. |
| `Authorization: Bearer <token>` header injection | ✅ Server saw the header (request reached the auth check). |
| `Content-Type: application/json` header | ✅ Server parsed body as JSON (validation errors include `path` and `expected` fields — only generated against a parsed payload). |
| Wire body shape: single-element JSON array of `{type, data, source}` envelopes | ✅ Server's batch validator returned a result keyed by `batchIndex: 0`, confirming it parsed a single-element batch. |
| Response body shape `{ ingested, deduped, ids }` | ✅ all three fields present (though field-order differs — see findings). |
| Exit-code mapping: 422 → exit 1 with stderr message; 2xx → exit 0 with stdout JSON | ✅ Both branches exercised. |

## Findings — contract divergences from wiremock fixtures

### 1. (High) `ai.task.completed.v1` schema is not what `tests/publish.rs` assumes

The wiremock tests use `--data '{"taskId":"t-1"}'` (`publish.rs:99`, `publish.rs:152`, `publish.rs:203`, etc) as a representative payload for `ai.task.completed.v1`. The **real schema** requires:

| Field | Type | Required? |
|---|---|---|
| `taskId` | string | yes |
| `sessionId` | string | yes |
| `startedAt` | string (ISO timestamp) | yes |
| `completedAt` | string (ISO timestamp) | yes |
| `outcome` | enum: `"success" \| "partial" \| "failed" \| "abandoned"` | yes |
| `callCount` | number | yes |
| `durationMs` | number | yes |

It also rejects unknown keys — `summary` (which I sent on the first attempt) returned `unrecognized_keys`. So the schema is **strict-shape** (no extra fields allowed), not open-shape.

**Impact on the test suite:** wiremock doesn't validate, so the existing 11 tests pass despite using a wrong payload shape. If the test plan ever moves to a real-server smoke (or to a wiremock fixture that mirrors the actual schema), every test fails until payloads are updated. This is exactly the contract-drift risk the `publish-spike-staging-smoke` fix doc warned about.

**Recommended follow-up:** when `nt-schemas` becomes a real `build.rs`-fetched bundle (per `cross-platform-cli-binary` Task 3a), the wiremock fixture payloads should be regenerated from the actual schemas — or, more pragmatically, the tests should switch to using `--type` ids that don't have strict shapes (e.g. a `meta.test.payload.v1` synthetic type used only for transport-level testing).

### 2. (Medium) Working payload for `ai.task.completed.v1`

```json
{
  "taskId": "rust-spike-smoke-001",
  "sessionId": "rust-spike-smoke-session-001",
  "startedAt": "2026-05-11T20:30:00.000Z",
  "completedAt": "2026-05-11T20:30:01.000Z",
  "outcome": "success",
  "callCount": 1,
  "durationMs": 1000
}
```

Documented here so the next person doing a smoke doesn't have to discover the schema by 422-error trial-and-error.

### 3. (Low) Response field order from server: alphabetical, not docstring-order

The TS reference and wiremock fixtures describe / use `{ ingested, deduped, ids }` (`tests/publish.rs:5`, `publish.rs:83`). The real server returns `{ deduped, ids, ingested }` (alphabetical):

```json
{"deduped":0,"ids":["4"],"ingested":1}
```

**Impact on tests:** zero. The wiremock tests access fields by name (`body["ingested"]`), not by position. The docstring at `tests/publish.rs:5` (`{ ingested, deduped, ids } response`) is now mildly misleading. No production code depends on response field order — `commands/publish.rs` just `serde_json::to_string`s the `Value` and prints it.

**Recommended follow-up:** none required; optionally fix the docstring comment to reflect reality.

### 4. (Low) Event ID format: small integer, not opaque string

The wiremock fixture uses `"ids": ["evt_abc123"]` (`tests/publish.rs:85`, `:112`, and `"ids": ["evt_1"]` at `:430`) implying an opaque alphanumeric event id. The real server returns `"ids": ["4"]` — a small integer encoded as a string. This is a representational difference (string in both cases) so nothing parses-incorrectly, but the fixture's "evt_" prefix is fictional.

**Impact:** none — tests don't pin the id format. If the server's id format ever changes (e.g. moves to UUID, or starts prefixing), neither the fixture comment nor any production code needs to change.

## Architectural validations holding

- `rustls + webpki-roots` strategy validated against a real cert — no need to revisit the TLS backend choice.
- `reqwest` 0.12 with `default-features = false` + `rustls-tls` features compiles and runs against staging — no surprise missing features.
- `tokio` current-thread runtime is fine for a single-shot publish; end-to-end latency was sub-second, not measured precisely.
- The post-`nt-cli-thin-edge-refactor` architecture (pure `build_envelope`, injected `HttpClient`, `Env` port) survived the real network path without regression — the wire format is byte-for-byte what wiremock asserted, even though the payload schema turned out to be different.

## Verdict

Task 14's stated goal ("single event to staging end-to-end") is now actually met. The publish path is real and works. The wiremock-based contract assertions are accurate at the transport level (HTTP headers, JSON envelope structure, response field-presence) but stale at the schema level (the payload type fixture is wrong against the real type's schema). Cleanest path forward:

1. **No urgent code fix needed.** The Rust binary worked end-to-end.
2. **Follow-up: switch wiremock fixtures to use a transport-only synthetic event type id** (or wait for `nt-schemas` build.rs work in Task 3a to regenerate real payloads). Tracked separately if the test fidelity matters enough — for now the wiremock contract validates *transport*, which is its job.
3. **Document the working payload** (above) — done in this section.
