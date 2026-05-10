import { byTypeId, type EventTypeId } from '@magic-ingredients/no-tickets-schemas';

export interface ValidationIssue {
  readonly path: string;
  readonly message: string;
}

/** Type guard — narrows `string` to `EventTypeId` when the id is a
 *  registered event type in the bundled schemas package. Callers gate
 *  publish/validation on this cheaply (no data parsing required) so an
 *  unknown-type error surfaces instead of being masked by a downstream
 *  JSON-parse or schema failure.
 *
 *  Uses Object.hasOwn so prototype names ('toString' / 'hasOwnProperty' /
 *  'valueOf') don't slip past the guard via the prototype chain. */
export function isKnownEventType(typeId: string): typeId is EventTypeId {
  return Object.hasOwn(byTypeId, typeId);
}

/** Validates an event payload against the bundled Zod schema for the
 *  given (already-narrowed) type id. Returns [] on success, ValidationIssue[]
 *  on schema failure.
 *
 *  Pre-condition: caller has gated the type id via `isKnownEventType` —
 *  the `EventTypeId` parameter is the narrowed type returned by that guard.
 *  This makes "unknown event type" structurally impossible at this layer
 *  and removes the dead branch a post-hoc runtime check would create. */
export function validateEventLocally(
  typeId: EventTypeId,
  data: unknown,
): readonly ValidationIssue[] {
  const schema = byTypeId[typeId];
  const result = schema.safeParse(data);
  if (result.success) return [];
  return result.error.issues.map((issue) => ({
    path: issue.path.join('.'),
    message: issue.message,
  }));
}

