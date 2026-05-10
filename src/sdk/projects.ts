// Stub — implementation lands in the GREEN commit.
//
// Phase 1 task: SDK — project registry loader + clientForProject factory.
// Reads `~/.notickets/config.json` `projects[name]`, joins it with the
// referenced profile to produce { token, apiUrl, authUrl }, and offers a
// one-line `clientForProject(name)` factory for production callers.

import type { ClientOptions } from '../transport/client.js';
import type { Client } from '../transport/client.js';

export interface ResolvedProjectAuth {
  readonly token: string;
  readonly apiUrl: string;
  readonly authUrl: string;
}

export class ProjectNotRegisteredError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ProjectNotRegisteredError';
  }
}

export function resolveProjectAuth(_name: string): ResolvedProjectAuth {
  throw new Error('resolveProjectAuth: not implemented');
}

export function clientForProject(_name: string, _overrides?: Partial<ClientOptions>): Client {
  throw new Error('clientForProject: not implemented');
}
