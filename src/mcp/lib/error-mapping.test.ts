import { describe, it, expect } from 'vitest';
import { ZodError } from 'zod';
import { mapErrorToToolResult } from './error-mapping.js';
import {
  UnknownEventTypeError,
  EventValidationError,
  PermissionDeniedError,
  ServerError,
  HttpError,
  MissingEtagError,
} from '../../transport/errors.js';

describe('mapErrorToToolResult', () => {
  it('maps UnknownEventTypeError exhaustively (no extra fields beyond code/message)', () => {
    const err = new UnknownEventTypeError('app.user.signed-up.v1', 0);

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'unknown_event_type',
        message: err.message,
      },
    });
  });

  it('maps EventValidationError to event_validation + first issue path', () => {
    const err = new EventValidationError('app.user.signed-up.v1', 0, [
      { path: ['data', 'email'], message: 'required' },
    ]);

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'event_validation',
        message: err.message,
        fieldPath: ['data', 'email'],
      },
    });
  });

  it('omits fieldPath when EventValidationError has no issues', () => {
    const err = new EventValidationError('app.x.v1', 0, []);

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'event_validation',
        message: err.message,
      },
    });
  });

  it('omits fieldPath when EventValidationError has an issue with an empty path', () => {
    const err = new EventValidationError('app.x.v1', 0, [
      { path: [], message: 'root error' },
    ]);

    const result = mapErrorToToolResult(err);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).not.toHaveProperty('fieldPath');
    }
  });

  it('maps PermissionDeniedError to permission_denied with the domain in the message', () => {
    const err = new PermissionDeniedError('app.thread');

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'permission_denied',
        message: err.message,
      },
    });
    expect(err.message).toContain('app.thread');
  });

  it('maps ServerError (5xx) to server_error with the status in the message', () => {
    const err = new ServerError(503, 'unavailable');

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'server_error',
        message: err.message,
      },
    });
    expect(err.message).toContain('503');
  });

  it('maps HttpError (unmapped 4xx) to http_error WITHOUT leaking the body field', () => {
    const err = new HttpError(418, { secret: 'tea' });

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'http_error',
        message: err.message,
      },
    });
    expect(err.message).toContain('418');
    if (!result.ok) {
      expect(JSON.stringify(result.error)).not.toContain('secret');
    }
  });

  it('maps MissingEtagError to missing_etag', () => {
    const err = new MissingEtagError('/v1/admin/event-types');

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'missing_etag',
        message: err.message,
      },
    });
  });

  it('maps ZodError to validation_error with the first issue path', () => {
    const zerr = new ZodError([
      {
        code: 'invalid_type',
        path: ['ingested'],
        message: 'expected number',
        expected: 'number',
        received: 'string',
      } as never,
    ]);

    const result = mapErrorToToolResult(zerr);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('validation_error');
      expect(result.error.fieldPath).toEqual(['ingested']);
    }
  });

  it('omits fieldPath when ZodError has no issues', () => {
    const zerr = new ZodError([]);

    const result = mapErrorToToolResult(zerr);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('validation_error');
      expect(result.error).not.toHaveProperty('fieldPath');
    }
  });

  it('omits fieldPath when ZodError issue has an empty path (root-level issue)', () => {
    const zerr = new ZodError([
      {
        code: 'invalid_type',
        path: [],
        message: 'root',
        expected: 'object',
        received: 'string',
      } as never,
    ]);

    const result = mapErrorToToolResult(zerr);

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).not.toHaveProperty('fieldPath');
    }
  });

  it('maps a generic Error to internal_error with the original message', () => {
    const err = new Error('boom');

    const result = mapErrorToToolResult(err);

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'internal_error',
        message: 'boom',
      },
    });
  });

  it('maps a non-Error throwable (string) to internal_error', () => {
    const result = mapErrorToToolResult('string thrown');

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'internal_error',
        message: 'string thrown',
      },
    });
  });
});
