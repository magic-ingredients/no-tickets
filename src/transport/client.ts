import { mapResponseError, ServerError } from './errors.js';

export type HttpMethod = 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';

export interface TransportLogger {
  debug(payload: unknown): void;
  warn(payload: unknown): void;
}

export interface ClientOptions {
  readonly baseUrl: string;
  readonly token: string;
  readonly fetch?: typeof fetch;
  readonly logger?: TransportLogger;
  readonly sleep?: (ms: number) => Promise<void>;
}

const MAX_ATTEMPTS = 3;
const BASE_BACKOFF_MS = 100;
const PUBLISH_PATH = '/v1/events';

const defaultSleep = (ms: number): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, ms);
  });

function joinUrl(baseUrl: string, path: string): string {
  const base = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl;
  const suffix = path.startsWith('/') ? path : `/${path}`;
  return `${base}${suffix}`;
}

function isIdempotent(method: HttpMethod, path: string): boolean {
  if (method !== 'GET') return false;
  // POST /v1/events is the only path where retry is forbidden, but we are
  // already in a GET branch — keep the explicit guard so a future mutation
  // that flips method handling cannot smuggle retries onto publish.
  return path !== PUBLISH_PATH;
}

async function readBody(response: Response): Promise<unknown> {
  if (response.status === 204) return undefined;
  const contentType = response.headers.get('content-type') ?? '';
  const text = await response.text();
  if (text.length === 0) return undefined;
  if (contentType.includes('application/json')) {
    try {
      return JSON.parse(text);
    } catch {
      return text;
    }
  }
  return text;
}

export class Client {
  readonly #baseUrl: string;
  readonly #token: string;
  readonly #fetch: typeof fetch;
  readonly #logger: TransportLogger | undefined;
  readonly #sleep: (ms: number) => Promise<void>;

  constructor(options: ClientOptions) {
    this.#baseUrl = options.baseUrl;
    this.#token = options.token;
    this.#fetch = options.fetch ?? fetch;
    this.#logger = options.logger;
    this.#sleep = options.sleep ?? defaultSleep;
  }

  async request<TResponse = unknown>(
    method: HttpMethod,
    path: string,
    body?: unknown,
  ): Promise<TResponse> {
    const url = joinUrl(this.#baseUrl, path);
    const init: RequestInit = {
      method,
      headers: this.#buildHeaders(body !== undefined),
    };
    if (body !== undefined) init.body = JSON.stringify(body);

    const maxAttempts = isIdempotent(method, path) ? MAX_ATTEMPTS : 1;
    let lastError: unknown;

    for (let attempt = 1; attempt <= maxAttempts; attempt++) {
      const start = Date.now();
      const response = await this.#fetch(url, init);
      const latencyMs = Date.now() - start;
      const responseBody = await readBody(response);

      this.#logger?.debug({ method, path, status: response.status, latencyMs });

      if (response.ok) {
        return responseBody as TResponse;
      }

      const error = mapResponseError(response.status, responseBody);
      const retryable = error instanceof ServerError && attempt < maxAttempts;
      if (!retryable) {
        throw error;
      }

      this.#logger?.warn({ method, path, status: response.status, attempt, latencyMs });
      lastError = error;
      await this.#sleep(BASE_BACKOFF_MS * 2 ** (attempt - 1));
    }

    throw lastError;
  }

  #buildHeaders(hasBody: boolean): Record<string, string> {
    const headers: Record<string, string> = {
      authorization: `Bearer ${this.#token}`,
    };
    if (hasBody) headers['content-type'] = 'application/json';
    return headers;
  }
}
