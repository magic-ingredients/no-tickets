import * as http from 'node:http';
import { URL } from 'node:url';
import { timingSafeEqual } from 'node:crypto';

function safeEqual(a: string, b: string): boolean {
  // Constant-time string compare. Bail early on length mismatch — that is not
  // sensitive (sender controls input length) and timingSafeEqual throws on
  // mismatched buffer lengths.
  if (a.length !== b.length) return false;
  return timingSafeEqual(Buffer.from(a), Buffer.from(b));
}

/** Pull a query value from a raw `?…` string without form-urlencoded `+` →
 *  space rewriting that URLSearchParams does. Preserves literal `+` in emails
 *  like `alice+test@example.com`. */
function getRawQueryValue(rawSearch: string, key: string): string | null {
  const pairs = rawSearch.replace(/^\?/, '').split('&');
  for (const pair of pairs) {
    const eq = pair.indexOf('=');
    if (eq < 0) continue;
    const rawKey = pair.slice(0, eq);
    const rawValue = pair.slice(eq + 1);
    try {
      if (decodeURIComponent(rawKey) === key) {
        return decodeURIComponent(rawValue);
      }
    } catch {
      // malformed percent-encoding — skip this pair
    }
  }
  return null;
}

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

    if (req.method !== 'GET') {
      res.writeHead(405);
      res.end();
      return;
    }

    const token = getRawQueryValue(url.search, 'token');
    const email = getRawQueryValue(url.search, 'email');
    const state = getRawQueryValue(url.search, 'state');

    if (!token || !email || !state || !safeEqual(state, expectedState)) {
      res.writeHead(400);
      res.end();
      return;
    }

    // Race guard: the auth server may have been closed between the request
    // arriving and us getting here. Don't tell the browser the auth was
    // accepted if the CLI promise was already rejected.
    if (settled) {
      res.writeHead(409);
      res.end();
      return;
    }

    settled = true;
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('Authentication successful. You can close this tab.');
    resolveCallback({ token, email });
    server.close();
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
      // Force-drop keep-alive sockets so test teardown doesn't flake.
      server.closeAllConnections?.();
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
