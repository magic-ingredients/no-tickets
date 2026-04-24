import * as http from 'node:http';
import { URL } from 'node:url';

export interface AuthServerOptions {
  /** CSRF nonce that the callback's `state` query param must match. */
  readonly expectedState: string;
  readonly timeoutMs?: number;
}

export interface AuthCallbackResult {
  readonly token: string;
  readonly email: string;
}

export interface AuthServerHandle {
  readonly port: number;
  readonly callbackPromise: Promise<AuthCallbackResult>;
  readonly close: () => Promise<void>;
}

const DEFAULT_TIMEOUT_MS = 120_000;

export async function startAuthServer(
  options: AuthServerOptions,
): Promise<AuthServerHandle> {
  const { expectedState } = options;
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;

  let resolveCallback: (result: AuthCallbackResult) => void;
  let rejectCallback: (error: Error) => void;
  let settled = false;

  const callbackPromise = new Promise<AuthCallbackResult>((resolve, reject) => {
    resolveCallback = resolve;
    rejectCallback = reject;
  });

  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? '/', `http://127.0.0.1`);

    if (url.pathname !== '/callback') {
      res.writeHead(404);
      res.end();
      return;
    }

    const token = url.searchParams.get('token');
    const email = url.searchParams.get('email');
    const state = url.searchParams.get('state');

    if (!token || !email || !state || state !== expectedState) {
      res.writeHead(400);
      res.end();
      return;
    }

    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('Authentication successful. You can close this tab.');

    if (!settled) {
      settled = true;
      resolveCallback({ token, email });
      server.close();
    }
  });

  const timeout = setTimeout(() => {
    if (!settled) {
      settled = true;
      rejectCallback(new Error('Authentication timed out — no callback received'));
      server.close();
    }
  }, timeoutMs);

  const close = async (): Promise<void> => {
    clearTimeout(timeout);
    if (!settled) {
      settled = true;
      rejectCallback(new Error('Auth server closed'));
    }
    return new Promise<void>((resolve) => {
      server.close(() => resolve());
    });
  };

  return new Promise<AuthServerHandle>((resolve, reject) => {
    server.on('error', reject);
    server.listen(0, '127.0.0.1', () => {
      const addr = server.address();
      if (!addr || typeof addr === 'string') {
        reject(new Error('Failed to get server address'));
        return;
      }
      resolve({ port: addr.port, callbackPromise, close });
    });
  });
}
