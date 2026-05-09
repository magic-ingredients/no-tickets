import * as http from 'node:http';
import { URL } from 'node:url';
import { timingSafeEqual } from 'node:crypto';

const LOGO_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 24 24" fill="none" stroke="url(#no-tickets-grad)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <defs>
    <linearGradient id="no-tickets-grad" x1="0" y1="0" x2="24" y2="24" gradientUnits="userSpaceOnUse">
      <stop offset="0%" stop-color="#14b8a6"/>
      <stop offset="100%" stop-color="#a855f7"/>
    </linearGradient>
  </defs>
  <path d="M2 9a3 3 0 0 1 0 6v2a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-2a3 3 0 0 1 0-6V7a2 2 0 0 0-2-2H4a2 2 0 0 0-2 2Z"/>
  <path d="m9.5 14.5 5-5"/>
</svg>`;

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function renderSuccessPage(appUrl: string): string {
  const safeUrl = escapeHtml(appUrl);
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>no-tickets — CLI authentication successful</title>
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <style>
    :root { color-scheme: light dark; }
    * { box-sizing: border-box; }
    html, body { height: 100%; margin: 0; }
    body {
      display: flex;
      align-items: center;
      justify-content: center;
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
      background: #f8fafc;
      color: #0f172a;
      padding: 2rem;
    }
    @media (prefers-color-scheme: dark) {
      body { background: #0b1220; color: #e2e8f0; }
      .divider span { background: #0b1220; color: #94a3b8; }
      .divider::before { background: #1e293b; }
    }
    main { text-align: center; max-width: 28rem; }
    .logo { display: inline-flex; margin-bottom: 1.5rem; }
    .wordmark {
      font-size: 1rem;
      font-weight: 600;
      letter-spacing: 0.02em;
      margin: 0 0 2rem;
      background: linear-gradient(135deg, #14b8a6, #a855f7);
      -webkit-background-clip: text;
      background-clip: text;
      color: transparent;
    }
    h1 { font-size: 1.25rem; margin: 0 0 0.5rem; font-weight: 600; }
    p { margin: 0; color: #475569; }
    @media (prefers-color-scheme: dark) { p { color: #94a3b8; } }
    .divider {
      position: relative;
      margin: 1.5rem 0;
      text-align: center;
    }
    .divider::before {
      content: "";
      position: absolute;
      top: 50%;
      left: 0;
      right: 0;
      height: 1px;
      background: #e2e8f0;
    }
    .divider span {
      position: relative;
      padding: 0 0.75rem;
      background: #f8fafc;
      color: #94a3b8;
      font-size: 0.875rem;
    }
    a.continue {
      display: inline-block;
      padding: 0.625rem 1.25rem;
      border-radius: 0.5rem;
      background: linear-gradient(135deg, #14b8a6, #a855f7);
      color: #fff;
      text-decoration: none;
      font-weight: 500;
      font-size: 0.9375rem;
    }
    a.continue:hover { opacity: 0.9; }
  </style>
</head>
<body>
  <main>
    <div class="logo">${LOGO_SVG}</div>
    <p class="wordmark">no-tickets</p>
    <h1>CLI authentication successful!</h1>
    <p>You can close this tab</p>
    <div class="divider"><span>or</span></div>
    <a class="continue" href="${safeUrl}">continue to no-tickets</a>
  </main>
</body>
</html>`;
}

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
  /** App origin (no path) used to render the "continue to no-tickets" link. */
  readonly appUrl: string;
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

export const DEFAULT_TIMEOUT_MS = 120_000;

export async function startAuthServer(
  options: AuthServerOptions,
): Promise<AuthServerHandle> {
  const { expectedState, appUrl } = options;
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
    res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
    res.end(renderSuccessPage(appUrl));
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
