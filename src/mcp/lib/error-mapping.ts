import { ZodError } from 'zod';
import {
  UnknownEventTypeError,
  EventValidationError,
  PermissionDeniedError,
  ServerError,
  HttpError,
  MissingEtagError,
} from '../../transport/errors.js';

export type StructuredToolErrorCode =
  | 'unknown_event_type'
  | 'event_validation'
  | 'permission_denied'
  | 'missing_etag'
  | 'server_error'
  | 'http_error'
  | 'validation_error'
  | 'internal_error';

export interface StructuredToolError {
  readonly code: StructuredToolErrorCode;
  readonly message: string;
  readonly fieldPath?: readonly (string | number)[];
}

export interface StructuredToolFailure {
  readonly ok: false;
  readonly error: StructuredToolError;
}

interface IssueLike {
  readonly path: readonly (string | number)[];
}

function firstIssuePath(issues: readonly IssueLike[]): readonly (string | number)[] | undefined {
  const firstIssue = issues[0];
  if (firstIssue === undefined) return undefined;
  if (firstIssue.path.length === 0) return undefined;
  return firstIssue.path;
}

function failure(
  code: StructuredToolErrorCode,
  message: string,
  fieldPath?: readonly (string | number)[],
): StructuredToolFailure {
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
 *  Anything else is mapped to `internal_error`. Empty-path issues do NOT
 *  leak as `fieldPath: []` — that key is omitted entirely so callers can
 *  branch on `'fieldPath' in error`. */
export function mapErrorToToolResult(err: unknown): StructuredToolFailure {
  if (err instanceof UnknownEventTypeError) {
    return failure('unknown_event_type', err.message);
  }
  if (err instanceof EventValidationError) {
    return failure('event_validation', err.message, firstIssuePath(err.issues));
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
    return failure('validation_error', err.message, firstIssuePath(err.issues));
  }
  if (err instanceof Error) {
    return failure('internal_error', err.message);
  }
  return failure('internal_error', String(err));
}
