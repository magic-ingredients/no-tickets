import { parseFiles } from '../core/parser.js';
import { toProjectEntities } from '../core/state.js';
import type { FileEntry, Push, Session } from '../core/types.js';

interface AssemblePushOptions {
  readonly files: readonly FileEntry[];
  readonly projectId: string;
  readonly session: Session;
  readonly timestamp?: string;
}

/**
 * Assemble a v2 Push payload from .notickets/ files and session.
 * Pure function — no I/O.
 */
export function assemblePush(options: AssemblePushOptions): Push {
  const parsed = parseFiles(options.files);
  const entities = toProjectEntities(parsed);

  return {
    projectId: options.projectId,
    timestamp: options.timestamp ?? new Date().toISOString(),
    session: options.session,
    ...(entities.length > 0 ? { project: { entities: [...entities] } } : {}),
  };
}

/**
 * Merge auto-enriched session into a push payload.
 * Does not overwrite an existing session.
 * Pure function.
 */
export function mergeSession(payload: Push, session: Session): Push {
  return {
    ...payload,
    session: payload.session ?? session,
  };
}
