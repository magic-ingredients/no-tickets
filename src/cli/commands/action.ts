import type { InteractionResponse } from '../../core/interaction.js';
import type { SubjectRef } from '../../core/subject.js';
import { PermissionDeniedError, HttpError } from '../../transport/errors.js';
import { resolveDataInput } from '../lib/data-input.js';

export interface ActionOptions {
  readonly interactionId: string;
  readonly input: string;
  readonly subjectType?: string;
  readonly subjectId?: string;
}

export interface ActionDeps {
  runInteraction(
    id: string,
    body: { readonly input: unknown; readonly subject?: SubjectRef },
  ): Promise<InteractionResponse>;
  readStdin(): Promise<string>;
  write(line: string): void;
  writeErr(line: string): void;
}

const EXIT_OK = 0;
const EXIT_VALIDATION = 1;
const EXIT_SERVER = 3;

export async function runAction(
  options: ActionOptions,
  deps: ActionDeps,
): Promise<number> {
  if (options.interactionId.trim().length === 0) {
    deps.writeErr('action: interaction id is required');
    return EXIT_VALIDATION;
  }

  let input: unknown;
  try {
    input = await resolveDataInput(options.input, { readStdin: deps.readStdin });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_VALIDATION;
  }

  const subject =
    options.subjectType !== undefined && options.subjectId !== undefined
      ? { type: options.subjectType, id: options.subjectId }
      : undefined;

  const body: { input: unknown; subject?: SubjectRef } = {
    input,
    ...(subject !== undefined && { subject }),
  };

  let response: InteractionResponse;
  try {
    response = await deps.runInteraction(options.interactionId, body);
  } catch (err) {
    if (err instanceof PermissionDeniedError) {
      deps.writeErr(`permission denied for domain "${err.domain}"`);
      return EXIT_VALIDATION;
    }
    if (err instanceof HttpError && err.status === 404) {
      deps.writeErr(`interaction "${options.interactionId}" not found`);
      return EXIT_VALIDATION;
    }
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_SERVER;
  }

  for (const ev of response.events) deps.write(ev.id);
  return EXIT_OK;
}
