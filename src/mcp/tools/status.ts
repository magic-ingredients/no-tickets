import { resolveAuth } from '../../sdk/auth.js';
import { toolSuccess, toolError, type ToolResult } from './types.js';

export function handleStatus(): ToolResult {
  try {
    const auth = resolveAuth();
    return toolSuccess({
      authenticated: true,
      source: auth.source,
      tokenType: auth.tokenType,
      apiUrl: process.env['NO_TICKETS_API_URL'] ?? 'https://api.no-tickets.com',
    });
  } catch {
    return toolError(new Error('Not authenticated. Set NO_TICKETS_TOKEN or run `npx no-tickets init`.'));
  }
}
