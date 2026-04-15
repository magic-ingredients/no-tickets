import type { SyncConfig, StateSnapshot, PushResult } from './core/types.js';
import type { TaskStatus, TaskPhase } from './commands/task-update.js';

interface ConnectResult {
  readonly success: boolean;
  readonly teamName?: string;
}

interface StatusResult {
  readonly connected: boolean;
  readonly teamName?: string;
}

interface TaskUpdatePayload {
  readonly status: TaskStatus;
  readonly phase?: TaskPhase;
  readonly commitSha?: string;
  readonly coverage?: number;
  readonly testsTotal?: number;
  readonly testsPassing?: number;
  readonly reviewVerdict?: string;
  readonly documentation?: boolean;
}

interface TaskUpdateResult {
  readonly success: boolean;
  readonly task?: Record<string, unknown>;
  readonly error?: string;
}

/**
 * Thin I/O shell over the core library.
 * Handles HTTP communication with no-tickets.com API.
 */
export class NoTicketsClient {
  private readonly config: SyncConfig;

  constructor(config: SyncConfig) {
    this.config = config;
  }

  async push(snapshot: StateSnapshot): Promise<PushResult> {
    try {
      const response = await fetch(`${this.config.apiUrl}/api/v1/snapshots`, {
        method: 'POST',
        headers: this.jsonHeaders(),
        body: JSON.stringify({
          teamId: this.config.teamId,
          projectId: this.config.projectId,
          snapshot,
        }),
      });

      if (!response.ok) {
        return { success: false, changesApplied: 0, eventsGenerated: 0 };
      }

      return parsePushResult(await response.json());
    } catch {
      return { success: false, changesApplied: 0, eventsGenerated: 0 };
    }
  }

  async connect(teamId: string): Promise<ConnectResult> {
    try {
      const response = await fetch(`${this.config.apiUrl}/api/v1/teams/${teamId}`, {
        headers: this.authHeaders(),
      });

      if (!response.ok) {
        return { success: false };
      }

      const data = parseTeamResponse(await response.json());
      return { success: true, teamName: data.name };
    } catch {
      return { success: false };
    }
  }

  async status(): Promise<StatusResult> {
    try {
      const response = await fetch(`${this.config.apiUrl}/api/v1/teams/${this.config.teamId}`, {
        headers: this.authHeaders(),
      });

      if (!response.ok) {
        return { connected: false };
      }

      const data = parseTeamResponse(await response.json());
      return { connected: true, teamName: data.name };
    } catch {
      return { connected: false };
    }
  }

  async taskUpdate(taskId: string, payload: TaskUpdatePayload): Promise<TaskUpdateResult> {
    try {
      const response = await fetch(`${this.config.apiUrl}/api/v1/tasks/${taskId}`, {
        method: 'PUT',
        headers: this.jsonHeaders(),
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const data = parseErrorResponse(await response.json());
        return { success: false, error: data.error };
      }

      return parseTaskUpdateResponse(await response.json());
    } catch (err) {
      return { success: false, error: err instanceof Error ? err.message : 'Unknown error' };
    }
  }

  private authHeaders(): Record<string, string> {
    return {
      'Authorization': `Bearer ${this.config.token}`,
    };
  }

  private jsonHeaders(): Record<string, string> {
    return {
      'Authorization': `Bearer ${this.config.token}`,
      'Content-Type': 'application/json',
    };
  }
}

function parsePushResult(data: unknown): PushResult {
  if (typeof data === 'object' && data !== null) {
    const obj = data as Record<string, unknown>;
    return {
      success: obj['success'] === true,
      changesApplied: typeof obj['changesApplied'] === 'number' ? obj['changesApplied'] : 0,
      eventsGenerated: typeof obj['eventsGenerated'] === 'number' ? obj['eventsGenerated'] : 0,
    };
  }
  return { success: false, changesApplied: 0, eventsGenerated: 0 };
}

function parseTeamResponse(data: unknown): { name: string } {
  if (typeof data === 'object' && data !== null) {
    const obj = data as Record<string, unknown>;
    return { name: typeof obj['name'] === 'string' ? obj['name'] : '' };
  }
  return { name: '' };
}

function parseTaskUpdateResponse(data: unknown): TaskUpdateResult {
  if (typeof data === 'object' && data !== null) {
    const obj = data as Record<string, unknown>;
    const task = typeof obj['task'] === 'object' && obj['task'] !== null
      ? obj['task'] as Record<string, unknown>
      : undefined;
    return { success: true, task };
  }
  return { success: true };
}

function parseErrorResponse(data: unknown): { error: string } {
  if (typeof data === 'object' && data !== null) {
    const obj = data as Record<string, unknown>;
    return { error: typeof obj['error'] === 'string' ? obj['error'] : 'Request failed' };
  }
  return { error: 'Request failed' };
}
