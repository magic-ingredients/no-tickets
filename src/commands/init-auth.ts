import { randomBytes } from 'node:crypto';
import { loadCredentials, saveCredentials } from '../sdk/credentials.js';
import { startAuthServer } from '../sdk/auth-server.js';

interface InitAuthOptions {
  readonly authUrl: string;
  readonly openBrowser: (url: string) => Promise<void>;
}

interface InitAuthResult {
  readonly token: string;
  readonly email: string;
  readonly isNewAuth: boolean;
}

const SESSION_DURATION_MS = 7 * 24 * 60 * 60 * 1000; // 7 days

function generateNonce(): string {
  return randomBytes(16).toString('hex');
}

function buildCallbackUrl(authUrl: string, port: number, code: string): string {
  const url = new URL(authUrl);
  url.searchParams.set('port', String(port));
  url.searchParams.set('code', code);
  return url.toString();
}

export async function resolveInitAuth(options: InitAuthOptions): Promise<InitAuthResult> {
  const existing = loadCredentials();
  if (existing) {
    return {
      token: existing.token,
      email: existing.email,
      isNewAuth: false,
    };
  }

  const code = generateNonce();
  const server = await startAuthServer({ expectedState: code });

  try {
    const callbackUrl = buildCallbackUrl(options.authUrl, server.port, code);
    await options.openBrowser(callbackUrl);

    const { token, email } = await server.callbackPromise;
    const expiresAt = new Date(Date.now() + SESSION_DURATION_MS).toISOString();

    saveCredentials(token, email, expiresAt);

    return { token, email, isNewAuth: true };
  } finally {
    await server.close();
  }
}

export type { InitAuthOptions, InitAuthResult };
