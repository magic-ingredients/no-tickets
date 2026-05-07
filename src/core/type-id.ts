// Type-ID grammar: {domain}.{entity}.{action}.v{version}
// Each segment: starts with a-z, may contain a-z, 0-9, _.
// Version: positive integer with no leading zero (v1, v2, ..., v100, ...).
export const TYPE_ID_REGEX = /^([a-z][a-z0-9_]*)\.([a-z][a-z0-9_]*)\.([a-z][a-z0-9_]*)\.v([1-9]\d*)$/;

export type TypeIdParts = Readonly<{
  domain: string;
  entity: string;
  action: string;
  version: number;
}>;

export function isTypeId(value: unknown): value is string {
  return typeof value === 'string' && TYPE_ID_REGEX.test(value);
}

export function parseTypeId(input: unknown): TypeIdParts | null {
  if (typeof input !== 'string') return null;
  const match = TYPE_ID_REGEX.exec(input);
  if (match === null) return null;
  const [, domain, entity, action, versionStr] = match;
  // Defensive narrowing under noUncheckedIndexedAccess. The regex's four
  // capture groups guarantee these are defined on a successful match; the
  // explicit check survives future regex edits that drop a group.
  if (
    domain === undefined ||
    entity === undefined ||
    action === undefined ||
    versionStr === undefined
  ) {
    return null;
  }
  const version = Number(versionStr);
  // [1-9]\d* permits arbitrarily long digit strings; reject if Number()
  // can't represent them exactly. Without this guard, parse → format → parse
  // is not stable for very large versions.
  if (!Number.isSafeInteger(version)) return null;
  return { domain, entity, action, version };
}

export function formatTypeId(parts: TypeIdParts): string {
  const formatted = `${parts.domain}.${parts.entity}.${parts.action}.v${parts.version}`;
  if (!TYPE_ID_REGEX.test(formatted)) {
    throw new Error(`formatTypeId: parts produced non-conforming type id: ${JSON.stringify(parts)}`);
  }
  return formatted;
}
