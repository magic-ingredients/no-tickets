import * as http from 'node:http';
import { URL } from 'node:url';

export interface AuthServerOptions {
  readonly timeoutMs?: number;
}

export interface AuthServerHandle {
  readonly port: number;
  readonly tokenPromise: Promise<string>;
  readonly close: () => Promise<void>;
}

const DEFAULT_TIMEOUT_MS = 120_000;

export async function startAuthServer(
  options: AuthServerOptions = {},
): Promise<AuthServerHandle> {
  const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;

  let resolveToken: (token: string) => void;
  let rejectToken: (error: Error) => void;
  let settled = false;

  const tokenPromise = new Promise<string>((resolve, reject) => {
    resolveToken = resolve;
    rejectToken = reject;
  });

  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? '/', `http://127.0.0.1`);

    if (url.pathname !== '/callback') {
      res.writeHead(404);
      res.end();
      return;
    }

    const token = url.searchParams.get('token');
    if (!token) {
      res.writeHead(400);
      res.end();
      return;
    }

    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('Authentication successful. You can close this tab.');

    if (!settled) {
      settled = true;
      resolveToken(token);
      server.close();
    }
  });

  const timeout = setTimeout(() => {
    if (!settled) {
      settled = true;
      rejectToken(new Error('Authentication timed out — no callback received'));
      server.close();
    }
  }, timeoutMs);

  const close = async (): Promise<void> => {
    clearTimeout(timeout);
    return new Promise<void>((resolve) => {
      server.close(() => resolve());
      // If server is already closed, the callback fires immediately
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
      resolve({ port: addr.port, tokenPromise, close });
    });
  });
}
