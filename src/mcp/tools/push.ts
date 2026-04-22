import { pushSchema } from '../../core/schemas.js';
import { detectAgent } from '../../agent-detect.js';
import { mergeSession } from '../../commands/push.js';
import { createApiClient } from '../../sdk/api-client.js';
import { resolveAuth } from '../../sdk/auth.js';
import { toolSuccess, type ToolResult } from './types.js';

export async function handlePush(payloadJson: string): Promise<ToolResult> {
  const raw = JSON.parse(payloadJson) as unknown;
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
