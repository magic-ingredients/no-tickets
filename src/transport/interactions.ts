import type { Client } from './client.js';
import {
  interactionRequestSchema,
  interactionResponseSchema,
  type InteractionResponse,
} from '../core/interaction.js';
import type { SubjectRef } from '../core/subject.js';

export interface RunInteractionBody<TInput = unknown> {
  readonly input: TInput;
  readonly subject?: SubjectRef;
}

export async function runInteraction<TInput = unknown>(
  client: Client,
  id: string,
  body: RunInteractionBody<TInput>,
): Promise<InteractionResponse> {
  interactionRequestSchema.parse({ id, input: body.input, subject: body.subject });

  const wireBody: Record<string, unknown> = { input: body.input };
  if (body.subject !== undefined) wireBody['subject'] = body.subject;

  const path = `/v1/interactions/${encodeURIComponent(id)}`;
  const response = await client.request<unknown>('POST', path, wireBody);
  return interactionResponseSchema.parse(response);
}
