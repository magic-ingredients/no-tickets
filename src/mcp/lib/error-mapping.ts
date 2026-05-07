import { ZodError } from 'zod';
import {
  UnknownEventTypeError,
  EventValidationError,
  PermissionDeniedError,
  ServerError,
  HttpError,
  MissingEtagError,
} from '../../transport/errors.js';

export interface StructuredToolError {
  readonly code: string;
  readonly message: string;
  readonly fieldPath?: readonly (string | number)[];
}

export type StructuredToolResult<T = unknown> =
  | { readonly ok: true; readonly result: T }
  | { readonly ok: false; readonly error: StructuredToolError };

function failure(code: string, message: string, fieldPath?: readonly (string | number)[]):
  StructuredToolResult<never> {
  return {
    ok: false,
    error: {
      code,
      message,
      ...(fieldPath !== undefined && { fieldPath }),
    },
  };
}

/** Convert a thrown error into a structured MCP tool result.
 *  Recognises the typed transport errors (Feature 2), MissingEtagError
 *  (Feature 3), and ZodError (local validation / response parsing).
 *  Anything else is mapped to `internal_error`. */
export function mapErrorToToolResult(err: unknown): StructuredToolResult<never> {
  if (err instanceof UnknownEventTypeError) {
    return failure('unknown_event_type', err.message);
  }
  if (err instanceof EventValidationError) {
    const firstIssue = err.issues[0];
    return failure(
      'event_validation',
      err.message,
      firstIssue !== undefined ? firstIssue.path : undefined,
    );
  }
  if (err instanceof PermissionDeniedError) {
    return failure('permission_denied', err.message);
  }
  if (err instanceof MissingEtagError) {
    return failure('missing_etag', err.message);
  }
  if (err instanceof ServerError) {
    return failure('server_error', err.message);
  }
  if (err instanceof HttpError) {
    return failure('http_error', err.message);
  }
  if (err instanceof ZodError) {
    const firstIssue = err.issues[0];
    return failure(
      'validation_error',
      err.message,
      firstIssue !== undefined ? firstIssue.path : undefined,
    );
  }
  if (err instanceof Error) {
    return failure('internal_error', err.message);
  }
  return failure('internal_error', String(err));
}
