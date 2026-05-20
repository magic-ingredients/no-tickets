# `nt` binary — structured error contract

This is the public, stable contract for how the `nt` binary signals failure to consumers (wrappers in any language). The shape is **additive-only across binary releases**: wrappers compiled against an old binary must continue to function against new ones. New variants get new exit codes (≥ 9) and new fields can be added, but existing variant names, exit codes, and field names never change or disappear.

## Surface

On failure, `nt` exits with a typed status code and writes a single line to stderr describing the failure. The line's format depends on whether stderr is a pipe or a TTY:

- **stderr is a pipe** (wrappers, CI, `nt ... 2> err.log`) — single-line JSON object
- **stderr is a TTY** (interactive use) — single human-readable line, no JSON braces

Auto-detection uses `std::io::IsTerminal`. Wrappers always get JSON; humans always get prose.

stdout is reserved for the command's success output and stays empty on failure.

## Exit codes and stderr JSON shapes

| Exit | Class | stderr JSON shape |
|---|---|---|
| 0 | success | (no stderr; stdout has the response) |
| 1 | `validation_error` | `{"error":"validation_error","typeId":"…","issues":[{"path":"…","message":"…"}], "batchIndex":N?}` |
| 2 | `unknown_event_type` | `{"error":"unknown_event_type","typeId":"…","suggestions":["…"]}` |
| 3 | `permission_denied` | `{"error":"permission_denied","domain":"…"}` |
| 4 | `transport_error` | `{"error":"transport_error","message":"…","retriable":true\|false}` |
| 5 | `not_authenticated` | `{"error":"not_authenticated","message":"…","storedHost"?:"…","currentHost"?:"…"}` |
| 6 | `project_not_registered` | `{"error":"project_not_registered","project":"…","knownProjects":["…"]}` |
| 7 | `usage` | `{"error":"usage","message":"…"}` |
| 8 | `token_rejected` | `{"error":"token_rejected","message":"…"}` |
| 64+ | reserved | future error classes |

### Field semantics

- **`"error"`** — the class string. The wrapper's primary discriminator. Pin this and match on it; never parse `message` to discriminate.
- **`typeId`** — fully-qualified event type id (`domain.entity.action.vN`). Always present in `validation_error` and `unknown_event_type`.
- **`batchIndex`** — present only in `validation_error` produced by JSONL batch mode; carries the 1-based line number that failed. Absent (not `null`) in single-event mode.
- **`issues[]`** — non-empty array; each `{path, message}` describes one schema failure. `path` is a JSON Pointer (`""` for the root).
- **`suggestions[]`** — possibly-empty fuzzy-match candidates for an unknown type id. Always an array (never `null`) so wrappers can iterate unconditionally.
- **`domain`** — string identifying the server resource that rejected the request. **As of Task 26 the only emitted value is `"events"`** — the wire layer only touches `/v1/events`. The field exists so wrappers can discriminate when a second domain lands (e.g. `tokens`); building against a single value today is forward-compatible.
- **`retriable`** — boolean. `true` for 5xx / network failure / **429 (rate-limit)** — the caller may retry after a delay. `false` for terminal 4xx that the caller should surface to the user directly.
- **`message`** — human-readable context. Pass through to the user, never parse for discrimination.
- **`storedHost`**, **`currentHost`** — optional, only present on a `not_authenticated` raised by ADR-0002 stored-session host mismatch (a context still emitted by `nt status`; `nt publish` no longer reads session credentials so this combination never appears under the publish exit codes). Both fields are absent (not `null`) when the failure is a plain missing-token case.
- **`project`**, **`knownProjects[]`** — the unrecognised project name and the locally-registered set. `knownProjects` is always an array (possibly empty).

> **`project_not_registered` vs `token_rejected`** — sister classes introduced by the `publish-uses-push-token` fix. `project_not_registered` (exit 6) fires **before transport**: the caller asked to publish under a `--project <name>` that isn't in `config.json` and `NO_TICKETS_TOKEN` isn't set. `token_rejected` (exit 8) fires **after transport**: the binary DID send a Bearer token, and the server returned 401. Both are distinct from `not_authenticated` (exit 5), which is reserved for missing-token failures on management-API commands (e.g. future `nt projects list`).

### Versioning policy

There is no `"version"` / `"schema"` field on the payload. The shape is governed by the **additive-only rule** above, enforced by tests in `crates/nt-cli/src/error.rs` and `crates/nt-cli/tests/structured_errors/`.

If a future change is genuinely breaking (a rename, a field type change, a removal), it MUST go through a new exit code (≥ 9) and class string. Wrappers parsing the existing eight classes continue to work; new wrappers opt in to the new class.

## Migration scope

As of the Task 26 landing, the contract is wired for:

- `nt publish` (single-event mode)
- `nt validate`

The following commands still emit free-text errors (no JSON shape) on stderr; they will be migrated in follow-up tasks:

- `nt init`
- `nt logout`
- `nt status`
- `nt token add | list | remove`
- `nt self-update`
- `nt publish --file` (JSONL batch mode)

Wrappers that drive only the migrated commands can rely on the JSON contract today. Wrappers that drive the unmigrated commands should fall back to parsing exit codes for those paths until the migrations land.

## Testing

The contract is pinned at three levels:

1. **Unit tests** in `crates/nt-cli/src/error.rs` (`#[cfg(test)] mod tests`) — every variant's `exit_code()`, `class()`, `to_json()` shape, `to_human()` line, and the `format_for(is_tty, ...)` / `emit_and_exit_code(...)` plumbing.
2. **Per-command integration tests** in `crates/nt-cli/tests/structured_errors/` — drive the binary via `assert_cmd` + `wiremock`, assert stderr parses as JSON with the documented shape and the right exit code per class.
3. **Cross-class invariants** — the exit-codes-are-distinct test guards against an accidental code collision when a new variant lands; the redirect-messages-are-distinct test (in `self_update`) guards against the same for per-manager strings.
