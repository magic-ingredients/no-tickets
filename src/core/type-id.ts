// Type-ID grammar: {domain}.{entity}.{action}.v{version}
// Each segment: starts with a-z, may contain a-z, 0-9, _. Version: integer, no leading zero, starts at v1.
export const TYPE_ID_REGEX = /^([a-z][a-z0-9_]*)\.([a-z][a-z0-9_]*)\.([a-z][a-z0-9_]*)\.v([1-9]\d*)$/;

export type TypeIdParts = Readonly<{
  domain: string;
  entity: string;
  action: string;
  version: number;
}>;

export function parseTypeId(input: unknown): TypeIdParts | null {
  if (typeof input !== 'string') return null;
  const match = TYPE_ID_REGEX.exec(input);
  if (match === null) return null;
  // Indices 1–4 are guaranteed by the regex's four capture groups; the
  // noUncheckedIndexedAccess assertions below document that contract.
  const [, domain, entity, action, version] = match;
  return {
    domain: domain as string,
    entity: entity as string,
    action: action as string,
    version: Number(version),
  };
}

export function formatTypeId(parts: TypeIdParts): string {
  return `${parts.domain}.${parts.entity}.${parts.action}.v${parts.version}`;
}
