import { resolveAuth, type TokenType } from '../sdk/auth.js';

export interface PushLocalConfig {
  readonly apiUrl: string;
  readonly teamId: string;
  readonly projectId: string;
}

export interface PushAuthConfig {
  readonly token: string;
  readonly apiUrl: string;
  readonly teamId: string | undefined;
  readonly projectId: string | undefined;
  readonly tokenType: TokenType;
}

export function buildPushAuth(localConfig: PushLocalConfig): PushAuthConfig {
  const resolved = resolveAuth();

  const includeIds = resolved.tokenType === 'session';

  return {
    token: resolved.token,
    apiUrl: localConfig.apiUrl,
    teamId: includeIds ? localConfig.teamId : undefined,
    projectId: includeIds ? localConfig.projectId : undefined,
    tokenType: resolved.tokenType,
  };
}
