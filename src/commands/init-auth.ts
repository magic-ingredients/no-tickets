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

// Placeholder until server-side /auth/cli returns email in the callback
const PLACEHOLDER_EMAIL = 'authenticated@no-tickets.com';

function buildCallbackUrl(authUrl: string, port: number): string {
  const url = new URL(authUrl);
  url.searchParams.set('callback_port', String(port));
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

  const server = await startAuthServer();

  try {
    const callbackUrl = buildCallbackUrl(options.authUrl, server.port);
    await options.openBrowser(callbackUrl);

    const token = await server.tokenPromise;
    const expiresAt = new Date(Date.now() + SESSION_DURATION_MS).toISOString();

    saveCredentials(token, PLACEHOLDER_EMAIL, expiresAt);

    return { token, email: PLACEHOLDER_EMAIL, isNewAuth: true };
  } finally {
    await server.close();
  }
}

export type { InitAuthOptions, InitAuthResult };
