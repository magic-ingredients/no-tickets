import { mapResponseError, ServerError } from './errors.js';
import { detectSource } from '../agent-detect.js';
import type { Source } from '../core/source.js';

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
  /** Override the auto-detected Source. When omitted, detectSource() runs
   *  lazily on first getSource() call and is cached for the client's lifetime. */
  readonly source?: Source;
}

const MAX_ATTEMPTS = 3;
const BASE_BACKOFF_MS = 100;

const defaultSleep = (ms: number): Promise<void> =>
  new Promise((resolve) => {
    setTimeout(resolve, ms);
  });

function joinUrl(baseUrl: string, path: string): string {
  const base = baseUrl.endsWith('/') ? baseUrl.slice(0, -1) : baseUrl;
  return `${base}${path}`;
}

async function readBody(response: Response): Promise<unknown> {
  if (response.status === 204) return undefined;
  const text = await response.text();
  if (text.length === 0) return undefined;
  return JSON.parse(text);
}

export class Client {
  readonly #baseUrl: string;
  readonly #token: string;
  readonly #fetch: typeof fetch;
  readonly #logger: TransportLogger | undefined;
  readonly #sleep: (ms: number) => Promise<void>;
  #source: Source | undefined;

  constructor(options: ClientOptions) {
    this.#baseUrl = options.baseUrl;
    this.#token = options.token;
    this.#fetch = options.fetch ?? fetch;
    this.#logger = options.logger;
    this.#sleep = options.sleep ?? defaultSleep;
    this.#source = options.source;
  }

  getSource(): Source {
    if (this.#source === undefined) {
      this.#source = detectSource();
    }
    return this.#source;
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
    // Stryker disable next-line ConditionalExpression: equivalent mutant —
    // JSON.stringify(undefined) returns undefined, so omitting this guard
    // would assign init.body = undefined, observationally identical.
    if (body !== undefined) init.body = JSON.stringify(body);

    const maxAttempts = method === 'GET' ? MAX_ATTEMPTS : 1;

    for (let attempt = 1; ; attempt++) {
      const start = Date.now();
      const response = await this.#fetch(url, init);
      const latencyMs = Date.now() - start;
      const responseBody = await readBody(response);

      this.#logger?.debug({ method, path, status: response.status, latencyMs });

      if (response.ok) {
        return responseBody as TResponse;
      }

      const error = mapResponseError(response.status, responseBody);
      const canRetry = error instanceof ServerError && attempt < maxAttempts;
      if (!canRetry) throw error;

      this.#logger?.warn({ method, path, status: response.status, attempt, latencyMs });
      await this.#sleep(BASE_BACKOFF_MS * 2 ** (attempt - 1));
    }
  }

  #buildHeaders(hasBody: boolean): Record<string, string> {
    const headers: Record<string, string> = {
      authorization: `Bearer ${this.#token}`,
    };
    if (hasBody) headers['content-type'] = 'application/json';
    return headers;
  }
}
