#!/usr/bin/env bash
#
# Seed the Product board for a staging demo. Publishes a believable but
# small dataset: 3 epics, 8 features spread across release stages
# (ideation / development / testing / review / done) + 1 archived,
# and 12 tasks under those features with a human/agent assignee mix.
#
# Re-runs are safe — every ID is deterministic, and the server dedupes
# `*.created` / `*.assigned` events by primary id and status_changed
# events by (entity, toStatus) — so a second run reports `deduped`
# rather than duplicating rows.
#
# Usage:
#   scripts/seed-product-demo.sh <auth-project> [<display-project-id>]
#
# Args:
#   <auth-project>        Local key registered via `no-tickets token add
#                         <name> <push-token>` that resolves to the push
#                         token used as Bearer on every publish. Passed
#                         as `no-tickets publish --project`. Required.
#   <display-project-id>  Value baked into the `projectId` field of every
#                         event payload — i.e. the board the events show
#                         up under server-side. Defaults to <auth-project>
#                         when omitted.
#
# Env knobs:
#   NO_TICKETS_ENV   — script default is `staging`. Set to `local` for
#                      the local stack, or `prod` for prod (both
#                      route through the `directories` crate's preset
#                      table in `crates/nt-cli/src/urls.rs`).
#   NT_BIN           — path to the no-tickets binary (default:
#                      `no-tickets` from PATH).
#
# Auth: <auth-project> must already have a registered push token via
# `no-tickets token add <name> <push-token>`. The push token is minted
# from the staging web UI. Alternatively, exporting NO_TICKETS_TOKEN as
# a CI escape hatch overrides whatever's on disk.
#
# `no-tickets init` is a separate flow that mints SESSION credentials
# for management commands; it does NOT register a push token under
# `--project <name>`. Use `token add` for that.

set -euo pipefail

AUTH_PROJECT="${1:?usage: scripts/seed-product-demo.sh <auth-project> [<display-project-id>]}"
DISPLAY_PROJECT="${2:-$AUTH_PROJECT}"
: "${NO_TICKETS_ENV:=staging}"
: "${NT_BIN:=no-tickets}"
export NO_TICKETS_ENV

count=0
publish() {
  local type="$1" data="$2"
  "$NT_BIN" publish --type "$type" --data "$data" --project "$AUTH_PROJECT" >/dev/null
  count=$((count + 1))
  printf '  %2d  %-38s %s\n' "$count" "$type" "$(printf '%s' "$data" | cut -c1-80)" >&2
}

echo "Seeding projectId=$DISPLAY_PROJECT via auth-project=$AUTH_PROJECT against NO_TICKETS_ENV=$NO_TICKETS_ENV" >&2

# ─── Epics ────────────────────────────────────────────────────────────
publish product.epic.created.v1 "$(printf '{"epicId":"epic-checkout-overhaul","projectId":"%s","title":"Checkout flow overhaul"}' "$DISPLAY_PROJECT")"
publish product.epic.created.v1 "$(printf '{"epicId":"epic-mobile-parity","projectId":"%s","title":"Mobile parity"}' "$DISPLAY_PROJECT")"
publish product.epic.created.v1 "$(printf '{"epicId":"epic-payments","projectId":"%s","title":"Payments integration"}' "$DISPLAY_PROJECT")"

# ─── Features (created at their final status; status_changed events
# below give a couple of them a visible transition history) ──────────
#
# ideation
publish product.feature.created.v1 "$(printf '{"featureId":"feat-address-autocomplete","projectId":"%s","title":"Address autocomplete","status":"ideation","parentId":"epic-checkout-overhaul"}' "$DISPLAY_PROJECT")"
publish product.feature.created.v1 "$(printf '{"featureId":"feat-returning-customer","projectId":"%s","title":"Returning-customer fast path","status":"ideation","parentId":"epic-checkout-overhaul"}' "$DISPLAY_PROJECT")"

# development
publish product.feature.created.v1 "$(printf '{"featureId":"feat-guest-checkout","projectId":"%s","title":"Guest checkout","status":"development","parentId":"epic-checkout-overhaul","assignee":"alice@acme.test","assigneeType":"human"}' "$DISPLAY_PROJECT")"
publish product.feature.created.v1 "$(printf '{"featureId":"feat-multi-currency","projectId":"%s","title":"Multi-currency pricing","status":"development","parentId":"epic-payments","assignee":"agent-claude","assigneeType":"agent"}' "$DISPLAY_PROJECT")"

# testing
publish product.feature.created.v1 "$(printf '{"featureId":"feat-apple-pay","projectId":"%s","title":"Apple Pay","status":"testing","parentId":"epic-payments","assignee":"bob@acme.test","assigneeType":"human"}' "$DISPLAY_PROJECT")"
publish product.feature.created.v1 "$(printf '{"featureId":"feat-mobile-cart","projectId":"%s","title":"Mobile cart UI","status":"testing","parentId":"epic-mobile-parity","assignee":"agent-codex","assigneeType":"agent"}' "$DISPLAY_PROJECT")"

# review
publish product.feature.created.v1 "$(printf '{"featureId":"feat-saved-cards","projectId":"%s","title":"Saved cards","status":"review","parentId":"epic-payments","assignee":"alice@acme.test","assigneeType":"human"}' "$DISPLAY_PROJECT")"

# done
publish product.feature.created.v1 "$(printf '{"featureId":"feat-cart-icon-badge","projectId":"%s","title":"Cart icon unread badge","status":"done","parentId":"epic-mobile-parity","assignee":"agent-claude","assigneeType":"agent"}' "$DISPLAY_PROJECT")"

# archived (created first, then archived in the event below)
publish product.feature.created.v1 "$(printf '{"featureId":"feat-legacy-checkout","projectId":"%s","title":"Legacy checkout (deprecated)","status":"done","parentId":"epic-checkout-overhaul"}' "$DISPLAY_PROJECT")"

# ─── Transition history (a couple of features get a status arc) ──────
# guest-checkout: ideation → development
publish product.feature.status_changed.v1 '{"featureId":"feat-guest-checkout","fromStatus":"ideation","toStatus":"development"}'
# apple-pay: development → testing
publish product.feature.status_changed.v1 '{"featureId":"feat-apple-pay","fromStatus":"development","toStatus":"testing"}'
# saved-cards: testing → review
publish product.feature.status_changed.v1 '{"featureId":"feat-saved-cards","fromStatus":"testing","toStatus":"review"}'
# cart-icon-badge: review → done (with a commit SHA so the dedupe key
# branch with `commitSha` gets exercised at least once)
publish product.feature.status_changed.v1 '{"featureId":"feat-cart-icon-badge","fromStatus":"review","toStatus":"done","commitSha":"deadbeef"}'

# ─── Archive ─────────────────────────────────────────────────────────
publish product.feature.archived.v1 '{"featureId":"feat-legacy-checkout","reason":"Superseded by epic-checkout-overhaul"}'

# ─── Tasks ───────────────────────────────────────────────────────────
# Spread across all 5 columns. Each task is created at its target
# status (mirrors the feature pattern). Two get a status_changed
# transition so the timeline isn't entirely flat.

# ideation (3)
publish product.task.created.v1 "$(printf '{"taskId":"task-postcode-lookup","featureId":"feat-address-autocomplete","title":"Pick postcode-lookup provider","status":"ideation"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-autocomplete-spec","featureId":"feat-address-autocomplete","title":"Draft autocomplete UX spec","status":"ideation"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-returning-survey","featureId":"feat-returning-customer","title":"User-research survey on returning checkout","status":"ideation"}')"

# development (3)
publish product.task.created.v1 "$(printf '{"taskId":"task-guest-form","featureId":"feat-guest-checkout","title":"Guest checkout form component","status":"development"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-currency-converter","featureId":"feat-multi-currency","title":"Currency-conversion service","status":"development"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-currency-display","featureId":"feat-multi-currency","title":"Currency-display formatter","status":"development"}')"

# testing (2)
publish product.task.created.v1 "$(printf '{"taskId":"task-applepay-handshake","featureId":"feat-apple-pay","title":"Apple Pay merchant-id handshake","status":"testing"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-mobile-cart-snapshot","featureId":"feat-mobile-cart","title":"Mobile cart screenshot tests","status":"testing"}')"

# review (2)
publish product.task.created.v1 "$(printf '{"taskId":"task-saved-cards-token","featureId":"feat-saved-cards","title":"Tokenisation review","status":"review"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-saved-cards-encryption","featureId":"feat-saved-cards","title":"At-rest encryption review","status":"review"}')"

# done (2)
publish product.task.created.v1 "$(printf '{"taskId":"task-cart-badge-impl","featureId":"feat-cart-icon-badge","title":"Cart badge component","status":"done"}')"
publish product.task.created.v1 "$(printf '{"taskId":"task-cart-badge-store","featureId":"feat-cart-icon-badge","title":"Unread-count store binding","status":"done"}')"

# ─── Task assignments (mix human + agent) ────────────────────────────
publish product.task.assigned.v1 '{"taskId":"task-guest-form","assignee":"alice@acme.test","assigneeType":"human"}'
publish product.task.assigned.v1 '{"taskId":"task-currency-converter","assignee":"agent-claude","assigneeType":"agent"}'
publish product.task.assigned.v1 '{"taskId":"task-currency-display","assignee":"agent-codex","assigneeType":"agent"}'
publish product.task.assigned.v1 '{"taskId":"task-applepay-handshake","assignee":"bob@acme.test","assigneeType":"human"}'
publish product.task.assigned.v1 '{"taskId":"task-mobile-cart-snapshot","assignee":"agent-claude","assigneeType":"agent"}'
publish product.task.assigned.v1 '{"taskId":"task-saved-cards-token","assignee":"alice@acme.test","assigneeType":"human"}'
publish product.task.assigned.v1 '{"taskId":"task-cart-badge-impl","assignee":"agent-claude","assigneeType":"agent"}'

# ─── Task transitions (just two, for timeline depth) ─────────────────
publish product.task.status_changed.v1 '{"taskId":"task-guest-form","fromStatus":"ideation","toStatus":"development"}'
publish product.task.status_changed.v1 '{"taskId":"task-cart-badge-impl","fromStatus":"review","toStatus":"done"}'

echo "Done — $count events published for project=$DISPLAY_PROJECT" >&2
