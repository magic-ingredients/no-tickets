export interface TokenCreateParams {
  readonly apiUrl: string;
  readonly sessionToken: string;
  readonly projectId: string;
  readonly label: string;
}

export interface TokenCreateResult {
  readonly success: boolean;
  readonly token?: string;
  readonly id?: string;
  readonly error?: string;
}

export interface TokenListParams {
  readonly apiUrl: string;
  readonly sessionToken: string;
}

export interface TokenListEntry {
  readonly id: string;
  readonly prefix: string;
  readonly label: string;
  readonly createdAt: string;
}

export interface TokenListResult {
  readonly success: boolean;
  readonly tokens: readonly TokenListEntry[];
  readonly error?: string;
}

export interface TokenRevokeParams {
  readonly apiUrl: string;
  readonly sessionToken: string;
  readonly tokenId: string;
}

export interface TokenRevokeResult {
  readonly success: boolean;
  readonly error?: string;
}

function authHeaders(sessionToken: string): Record<string, string> {
  return {
    'Authorization': `Bearer ${sessionToken}`,
  };
}

function jsonHeaders(sessionToken: string): Record<string, string> {
  return {
    'Authorization': `Bearer ${sessionToken}`,
    'Content-Type': 'application/json',
  };
}

function parseErrorMessage(data: unknown): string {
  if (typeof data === 'object' && data !== null && 'error' in data) {
    const errorVal = (data as Record<string, unknown>)['error'];
    if (typeof errorVal === 'string') return errorVal;
  }
  return 'Request failed';
}

async function safeParseErrorBody(response: Response): Promise<string> {
  try {
    const data: unknown = await response.json();
    return parseErrorMessage(data);
  } catch {
    return `Request failed with status ${response.status}`;
  }
}

export async function createToken(params: TokenCreateParams): Promise<TokenCreateResult> {
  try {
    const response = await fetch(`${params.apiUrl}/api/v1/tokens`, {
      method: 'POST',
      headers: jsonHeaders(params.sessionToken),
      body: JSON.stringify({
        projectId: params.projectId,
        label: params.label,
      }),
    });

    if (!response.ok) {
      const error = await safeParseErrorBody(response);
      return { success: false, error };
    }

    const data = (await response.json()) as Record<string, unknown>;
    return {
      success: true,
      token: typeof data['token'] === 'string' ? data['token'] : undefined,
      id: typeof data['id'] === 'string' ? data['id'] : undefined,
    };
  } catch (err) {
    return { success: false, error: err instanceof Error ? err.message : 'Unknown error' };
  }
}

export async function listTokens(params: TokenListParams): Promise<TokenListResult> {
  try {
    const response = await fetch(`${params.apiUrl}/api/v1/tokens`, {
      headers: authHeaders(params.sessionToken),
    });

    if (!response.ok) {
      const error = await safeParseErrorBody(response);
      return { success: false, tokens: [], error };
    }

    const data = (await response.json()) as Record<string, unknown>;
    const rawTokens = Array.isArray(data['tokens']) ? data['tokens'] : [];
    const tokens: TokenListEntry[] = rawTokens
      .filter((t): t is Record<string, unknown> => typeof t === 'object' && t !== null)
      .map((t) => ({
        id: typeof t['id'] === 'string' ? t['id'] : '',
        prefix: typeof t['prefix'] === 'string' ? t['prefix'] : '',
        label: typeof t['label'] === 'string' ? t['label'] : '',
        createdAt: typeof t['createdAt'] === 'string' ? t['createdAt'] : '',
      }));

    return { success: true, tokens };
  } catch (err) {
    return { success: false, tokens: [], error: err instanceof Error ? err.message : 'Unknown error' };
  }
}

export async function revokeToken(params: TokenRevokeParams): Promise<TokenRevokeResult> {
  try {
    const response = await fetch(`${params.apiUrl}/api/v1/tokens/${encodeURIComponent(params.tokenId)}`, {
      method: 'DELETE',
      headers: authHeaders(params.sessionToken),
    });

    if (!response.ok) {
      const error = await safeParseErrorBody(response);
      return { success: false, error };
    }

    return { success: true };
  } catch (err) {
    return { success: false, error: err instanceof Error ? err.message : 'Unknown error' };
  }
}
