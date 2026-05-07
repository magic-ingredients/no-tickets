import type { EventTypeSpec } from '../../../registry/client.js';
import type { EventsListOptions } from '../../../registry/index.js';

export interface EventListDeps {
  listEvents(options?: EventsListOptions): Promise<readonly EventTypeSpec[]>;
  write(line: string): void;
  writeErr(line: string): void;
}

export interface EventListOptions {
  readonly domain?: string;
  readonly deprecated?: boolean;
}

const DEPRECATED_MARKER = ' (deprecated)';
const INDENT = '  ';

function isDeprecated(type: EventTypeSpec): boolean {
  return type.deprecatedAt !== null && type.deprecatedAt !== undefined;
}

function buildFacadeOptions(options: EventListOptions): EventsListOptions {
  const facadeOptions: { domain?: string; deprecated?: boolean } = {};
  if (options.domain !== undefined) facadeOptions.domain = options.domain;
  if (options.deprecated !== undefined) facadeOptions.deprecated = options.deprecated;
  return facadeOptions;
}

function groupByDomain(types: readonly EventTypeSpec[]): Map<string, EventTypeSpec[]> {
  const groups = new Map<string, EventTypeSpec[]>();
  for (const type of types) {
    const existing = groups.get(type.domain);
    if (existing !== undefined) {
      existing.push(type);
    } else {
      groups.set(type.domain, [type]);
    }
  }
  return groups;
}

export async function runEventList(
  options: EventListOptions,
  deps: EventListDeps,
): Promise<number> {
  let types: readonly EventTypeSpec[];
  try {
    types = await deps.listEvents(buildFacadeOptions(options));
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return 1;
  }

  if (types.length === 0) {
    deps.write('No event types in cache.');
    return 0;
  }

  const groups = groupByDomain(types);
  const sortedDomains = Array.from(groups.keys()).sort();

  for (const domain of sortedDomains) {
    deps.write(domain);
    const groupTypes = groups.get(domain) ?? [];
    for (const type of groupTypes) {
      const marker = isDeprecated(type) ? DEPRECATED_MARKER : '';
      deps.write(`${INDENT}${type.id}${marker}`);
    }
  }
  return 0;
}
