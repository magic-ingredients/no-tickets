import { pushSchema } from '../../core/schemas.js';
import { detectAgent } from '../../agent-detect.js';
import { mergeSession } from '../../commands/push.js';
import { createApiClient } from '../../sdk/api-client.js';
import { resolveAuth } from '../../sdk/auth.js';
import { toolSuccess, toolError, type ToolResult } from './types.js';

export async function handlePush(payloadJson: string): Promise<ToolResult> {
  let raw: unknown;
  try {
    raw = JSON.parse(payloadJson);
  } catch {
    return toolError(new Error('Invalid JSON in push payload'));
  }

  const validated = pushSchema.parse(raw);

  const session = detectAgent();
  const payload = mergeSession(validated, session);

  const auth = resolveAuth();
  const client = createApiClient({
    token: auth.token,
    apiUrl: process.env['NO_TICKETS_API_URL'] ?? 'https://api.no-tickets.com',
  });

  const result = await client.push(payload);
  return toolSuccess(result);
}
