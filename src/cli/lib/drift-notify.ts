import type { CacheFile } from '../../registry/cache.js';
import type { AwaitRefreshResult } from '../../registry/refresh.js';

export interface DriftNotifyDeps {
  readPriorCache(): CacheFile | null;
  readPostCache(): CacheFile | null;
  writeErr(line: string): void;
}

export interface DriftNotifyOptions {
  readonly quiet?: boolean;
}

const MAX_LISTED = 3;

function diffNewIds(prior: CacheFile, next: CacheFile): readonly string[] {
  const priorIds = new Set(prior.types.map((t) => t.id));
  return next.types.map((t) => t.id).filter((id) => !priorIds.has(id));
}

function isQuiet(options: DriftNotifyOptions): boolean {
  if (options.quiet === true) return true;
  return process.env['NO_TICKETS_QUIET'] === '1';
}

function safeRead(read: () => CacheFile | null): CacheFile | null {
  // The cache layer returns null on corrupt / unreadable cache files, but a
  // misbehaving read implementation could still throw. notifyDrift's
  // contract is "never blocks; never throws to the caller" — swallow
  // unexpected read errors and treat them as "no cache".
  try {
    return read();
  } catch {
    return null;
  }
}

/** Print a one-line stderr drift summary when a refresh introduced new
 *  event types since the last sync. Suppressed by --quiet or
 *  NO_TICKETS_QUIET=1. Never blocks; never throws to the caller. */
export async function notifyDrift(
  refresh: Promise<AwaitRefreshResult>,
  options: DriftNotifyOptions,
  deps: DriftNotifyDeps,
): Promise<void> {
  if (isQuiet(options)) return;

  const prior = safeRead(deps.readPriorCache);
  if (prior === null) return;

  let result: AwaitRefreshResult;
  try {
    result = await refresh;
  } catch {
    return;
  }
  if (result.status !== 'updated') return;

  const next = safeRead(deps.readPostCache);
  if (next === null) return;

  const newIds = diffNewIds(prior, next);
  if (newIds.length === 0) return;

  const visible = newIds.slice(0, MAX_LISTED).join(', ');
  const suffix = newIds.length > MAX_LISTED ? ', ...' : '';
  deps.writeErr(`ℹ ${newIds.length} new event types since last sync: ${visible}${suffix}`);
}
