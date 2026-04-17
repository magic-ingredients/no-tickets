import type { BoardState, FeedEvent, AssigneeType } from '../core/types.js';

interface ApiClientConfig {
  readonly token: string;
  readonly apiUrl: string;
}

interface CreateEpicParams {
  readonly projectId: string;
  readonly title: string;
  readonly description?: string;
}

interface CreateFeatureParams {
  readonly projectId: string;
  readonly epicId: string;
  readonly title: string;
  readonly description?: string;
}

interface CreateFixParams {
  readonly projectId: string;
  readonly epicId: string;
  readonly title: string;
  readonly description?: string;
}

interface UpdateFeatureParams {
  readonly projectId: string;
  readonly featureId: string;
  readonly title?: string;
  readonly description?: string;
}

interface MoveToPhaseParams {
  readonly projectId: string;
  readonly featureId: string;
  readonly phase: string;
}

interface AssignFeatureParams {
  readonly projectId: string;
  readonly featureId: string;
  readonly assignee: string;
  readonly assigneeType: AssigneeType;
}

interface BreakDownParams {
  readonly projectId: string;
  readonly featureId: string;
  readonly context?: string;
}

interface ApiClient {
  getBoard(projectId: string): Promise<BoardState>;
  getFeed(projectId: string): Promise<readonly FeedEvent[]>;
  createEpic(params: CreateEpicParams): Promise<unknown>;
  createFeature(params: CreateFeatureParams): Promise<unknown>;
  createFix(params: CreateFixParams): Promise<unknown>;
  updateFeature(params: UpdateFeatureParams): Promise<unknown>;
  moveToPhase(params: MoveToPhaseParams): Promise<unknown>;
  assignFeature(params: AssignFeatureParams): Promise<unknown>;
  breakDown(params: BreakDownParams): Promise<unknown>;
}

function hasErrorField(value: unknown): value is { error: unknown } {
  return typeof value === 'object' && value !== null && 'error' in value;
}

async function request(apiUrl: string, token: string, path: string, options?: RequestInit): Promise<unknown> {
  const url = `${apiUrl}${path}`;
  const headers: Record<string, string> = {
    'Authorization': `Bearer ${token}`,
  };
  if (options?.body) {
    headers['Content-Type'] = 'application/json';
  }

  const response = await fetch(url, {
    ...options,
    headers: {
      ...headers,
      ...options?.headers as Record<string, string> | undefined,
    },
  });

  const text = await response.text();
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    if (!response.ok) {
      throw new Error(`${response.status}: ${text || 'Request failed'}`);
    }
    return text;
  }

  if (!response.ok) {
    const message = hasErrorField(body)
      ? String(body.error)
      : 'Request failed';
    throw new Error(`${response.status}: ${message}`);
  }

  return body;
}

export function createApiClient(config: ApiClientConfig): ApiClient {
  const { token, apiUrl } = config;

  return {
    async getBoard(projectId) {
      return request(apiUrl, token, `/api/v1/board/${projectId}`) as Promise<BoardState>;
    },

    async getFeed(projectId) {
      return request(apiUrl, token, `/api/v1/feed/${projectId}`) as Promise<readonly FeedEvent[]>;
    },

    async createEpic(params) {
      return request(apiUrl, token, '/api/v1/epics', {
        method: 'POST',
        body: JSON.stringify(params),
      });
    },

    async createFeature(params) {
      return request(apiUrl, token, '/api/v1/features', {
        method: 'POST',
        body: JSON.stringify(params),
      });
    },

    async createFix(params) {
      return request(apiUrl, token, '/api/v1/fixes', {
        method: 'POST',
        body: JSON.stringify(params),
      });
    },

    async updateFeature(params) {
      const { featureId, ...body } = params;
      return request(apiUrl, token, `/api/v1/features/${featureId}`, {
        method: 'PATCH',
        body: JSON.stringify(body),
      });
    },

    async moveToPhase(params) {
      const { featureId, ...body } = params;
      return request(apiUrl, token, `/api/v1/features/${featureId}/move`, {
        method: 'POST',
        body: JSON.stringify(body),
      });
    },

    async assignFeature(params) {
      const { featureId, ...body } = params;
      return request(apiUrl, token, `/api/v1/features/${featureId}/assign`, {
        method: 'POST',
        body: JSON.stringify(body),
      });
    },

    async breakDown(params) {
      return request(apiUrl, token, '/api/v1/break-down', {
        method: 'POST',
        body: JSON.stringify(params),
      });
    },
  };
}

export type { ApiClient, ApiClientConfig };
