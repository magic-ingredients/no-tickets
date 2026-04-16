import { loadCredentials } from './credentials.js';

export type TokenType = 'push' | 'session' | 'unknown';
export type AuthSource = 'env' | 'credentials';

export interface ResolvedAuth {
  readonly token: string;
  readonly source: AuthSource;
  readonly tokenType: TokenType;
}

function detectTokenType(token: string): TokenType {
  if (token.startsWith('nt_push_')) return 'push';
  if (token.startsWith('nt_session_')) return 'session';
  return 'unknown';
}

export function resolveAuth(): ResolvedAuth {
  const envToken = process.env['NO_TICKETS_TOKEN'];
  if (envToken) {
    return {
      token: envToken,
      source: 'env',
      tokenType: detectTokenType(envToken),
    };
  }

  const stored = loadCredentials();
  if (stored) {
    return {
      token: stored.token,
      source: 'credentials',
      tokenType: detectTokenType(stored.token),
    };
  }

  throw new Error('Not authenticated. Run `npx no-tickets init` to authenticate');
}
