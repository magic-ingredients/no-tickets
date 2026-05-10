import type { EventTypeSpec } from '../../registry/client.js';
import type { Subject, SubjectRef } from '../../core/subject.js';
import type { Source } from '../../core/source.js';
import type {
  PublishEvent,
  PublishResponse,
} from '../../transport/events.js';
import type { InteractionResponse } from '../../core/interaction.js';
import type { EventsListOptions } from '../../registry/index.js';
import { synthesiseExample } from '../../lib/example-synth.js';

export interface ToolHandlerDeps {
  readonly events: {
    list(options?: EventsListOptions): Promise<readonly EventTypeSpec[]>;
    describe(typeId: string): Promise<EventTypeSpec | null>;
  };
  readonly subjectsCreate: (subject: Subject) => Promise<Subject>;
  readonly publishEvents: (
    events: readonly PublishEvent[],
  ) => Promise<PublishResponse>;
  readonly runInteraction: (
    id: string,
    body: { readonly input: unknown; readonly subject?: SubjectRef },
  ) => Promise<InteractionResponse>;
  readonly source: Source;
}

// list_event_types -----------------------------------------------------------

export interface ListEventTypesArgs {
  readonly domain?: string;
  readonly deprecated?: boolean;
}

export interface ListEventTypesResultRow {
  readonly id: string;
  readonly domain: string;
  readonly entity: string;
  readonly action: string;
  /** Version string from the event-type id suffix, e.g. "v1". Matches the
   *  server's wire shape; pin the immutable .vN dimension at the type level. */
  readonly version: string;
}

export interface ListEventTypesResult {
  readonly types: readonly ListEventTypesResultRow[];
}

function buildListOptions(args: ListEventTypesArgs): EventsListOptions {
  return {
    ...(args.domain !== undefined && { domain: args.domain }),
    ...(args.deprecated !== undefined && { deprecated: args.deprecated }),
  };
}

export async function handleListEventTypes(
  args: ListEventTypesArgs,
  deps: ToolHandlerDeps,
): Promise<ListEventTypesResult> {
  const types = await deps.events.list(buildListOptions(args));
  return {
    types: types.map((t) => ({
      id: t.id,
      domain: t.domain,
      entity: t.entity,
      action: t.action,
      version: t.version,
    })),
  };
}

// describe_event_type --------------------------------------------------------

export interface DescribeEventTypeArgs {
  readonly id: string;
}

export interface DescribeEventTypeResult {
  readonly id: string;
  readonly schema: Record<string, unknown>;
  readonly example: unknown;
  readonly dedupe_strategy?: string;
  readonly retention_days?: number;
  readonly ui_hints?: Record<string, unknown>;
  readonly deprecated_at?: string | null;
}

export async function handleDescribeEventType(
  args: DescribeEventTypeArgs,
  deps: ToolHandlerDeps,
): Promise<DescribeEventTypeResult> {
  const spec = await deps.events.describe(args.id);
  if (spec === null) {
    throw new Error(`event type "${args.id}" not found`);
  }
  // The detail endpoint includes schema; the list endpoint omits it.
  // Describe always hits detail, so absence here is a server-contract
  // violation worth surfacing loudly rather than rendering an empty
  // example silently.
  if (spec.schema === undefined) {
    throw new Error(
      `event type "${args.id}" detail response is missing the schema field — server contract violation`,
    );
  }
  return {
    id: spec.id,
    schema: spec.schema,
    example: synthesiseExample(spec.schema),
    ...(spec.dedupeStrategy !== undefined && { dedupe_strategy: spec.dedupeStrategy }),
    ...(spec.retentionDays !== undefined && { retention_days: spec.retentionDays }),
    ...(spec.uiHints !== undefined && { ui_hints: spec.uiHints }),
    ...(spec.deprecatedAt !== undefined && { deprecated_at: spec.deprecatedAt }),
  };
}

// publish_event --------------------------------------------------------------

export interface PublishEventArgs {
  readonly type: string;
  readonly data: Record<string, unknown>;
  readonly subject?: SubjectRef;
  readonly occurred_at?: string;
  readonly parent_event_id?: string;
  readonly trace_id?: string;
  readonly dedupe_key?: string;
}

export interface PublishEventResult {
  readonly id: string;
  readonly deduped: boolean;
}

export async function handlePublishEvent(
  args: PublishEventArgs,
  deps: ToolHandlerDeps,
): Promise<PublishEventResult> {
  const event: PublishEvent = {
    type: args.type,
    data: args.data,
    source: deps.source,
    ...(args.subject !== undefined && { subject: args.subject }),
    ...(args.occurred_at !== undefined && { occurredAt: args.occurred_at }),
    ...(args.parent_event_id !== undefined && { parentEventId: args.parent_event_id }),
    ...(args.trace_id !== undefined && { traceId: args.trace_id }),
    ...(args.dedupe_key !== undefined && { dedupeKey: args.dedupe_key }),
  };
  const response = await deps.publishEvents([event]);
  const id = response.ids[0];
  if (id === undefined) {
    // Defensive: a 200 from POST /v1/events should always carry exactly one
    // id when we sent one event. An empty ids array is a server-contract
    // violation — surface it loudly rather than handing the agent a blank
    // string it might use as parent_event_id later.
    throw new Error(
      'publish_event: server response is missing the event id (expected exactly one for a singular publish)',
    );
  }
  const deduped = response.ingested === 0 && response.deduped > 0;
  return { id, deduped };
}

// run_interaction ------------------------------------------------------------

export interface RunInteractionArgs {
  readonly id: string;
  readonly input: Record<string, unknown>;
  readonly subject?: SubjectRef;
}

export interface RunInteractionResult {
  readonly events: readonly { readonly id: string; readonly type: string }[];
}

export async function handleRunInteraction(
  args: RunInteractionArgs,
  deps: ToolHandlerDeps,
): Promise<RunInteractionResult> {
  const body = args.subject !== undefined
    ? { input: args.input, subject: args.subject }
    : { input: args.input };
  const response = await deps.runInteraction(args.id, body);
  return { events: response.events };
}

// create_subject -------------------------------------------------------------

export interface CreateSubjectArgs {
  readonly type: string;
  readonly external_id: string;
  readonly display_name: string;
  readonly metadata?: Record<string, unknown>;
}

export interface CreateSubjectResult {
  readonly type: string;
  readonly id: string;
}

export async function handleCreateSubject(
  args: CreateSubjectArgs,
  deps: ToolHandlerDeps,
): Promise<CreateSubjectResult> {
  const subject: Subject = {
    type: args.type,
    externalId: args.external_id,
    displayName: args.display_name,
    ...(args.metadata !== undefined && { metadata: args.metadata }),
  };
  const created = await deps.subjectsCreate(subject);
  return { type: created.type, id: created.externalId };
}
