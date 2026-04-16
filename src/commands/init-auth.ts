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
    const callbackUrl = `${options.authUrl}?callback_port=${server.port}`;
    await options.openBrowser(callbackUrl);

    const token = await server.tokenPromise;
    const email = 'user@no-tickets.com';
    const expiresAt = new Date(Date.now() + SESSION_DURATION_MS).toISOString();

    saveCredentials(token, email, expiresAt);

    return { token, email, isNewAuth: true };
  } finally {
    await server.close();
  }
}

export type { InitAuthOptions, InitAuthResult };
