import { loadCredentials } from './credentials.js';

export type TokenType = 'push' | 'session' | 'unknown';
export type AuthSource = 'env' | 'credentials';

export interface ResolvedAuth {
  readonly token: string;
  readonly source: AuthSource;
  readonly tokenType: TokenType;
}

export interface AuthStatus {
  readonly authenticated: true;
  readonly source: AuthSource;
  readonly tokenType: TokenType;
  readonly apiUrl: string;
}

export const DEFAULT_API_URL = 'https://api.no-tickets.com';
export const NOT_AUTHENTICATED_MESSAGE =
  'Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.';

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

  throw new Error(NOT_AUTHENTICATED_MESSAGE);
}

/**
 * Resolve auth and shape it into a user-facing status payload.
 * Shared by the CLI `status` command and the MCP `status` tool.
 */
export function describeAuthStatus(): AuthStatus {
  const auth = resolveAuth();
  return {
    authenticated: true,
    source: auth.source,
    tokenType: auth.tokenType,
    apiUrl: process.env['NO_TICKETS_API_URL'] ?? DEFAULT_API_URL,
  };
}
