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
  it('maps UnknownEventTypeError to ok:false with code "unknown_event_type"', () => {
    const result = mapErrorToToolResult(
      new UnknownEventTypeError('app.user.signed-up.v1', 0),
    );

    expect(result).toEqual({
      ok: false,
      error: {
        code: 'unknown_event_type',
        message: expect.stringContaining('app.user.signed-up.v1'),
      },
    });
  });

  it('maps EventValidationError to ok:false with code "event_validation" and the first issue path', () => {
    const result = mapErrorToToolResult(
      new EventValidationError('app.user.signed-up.v1', 0, [
        { path: ['data', 'email'], message: 'required' },
      ]),
    );

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('event_validation');
      expect(result.error.fieldPath).toEqual(['data', 'email']);
      expect(result.error.message).toContain('app.user.signed-up.v1');
    }
  });

  it('omits fieldPath when EventValidationError has no issues', () => {
    const result = mapErrorToToolResult(
      new EventValidationError('app.x.v1', 0, []),
    );

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error).not.toHaveProperty('fieldPath');
    }
  });

  it('maps PermissionDeniedError to code "permission_denied" with the domain in the message', () => {
    const result = mapErrorToToolResult(new PermissionDeniedError('app.thread'));

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('permission_denied');
      expect(result.error.message).toContain('app.thread');
    }
  });

  it('maps ServerError (5xx) to code "server_error" with the status in the message', () => {
    const result = mapErrorToToolResult(new ServerError(503, 'unavailable'));

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('server_error');
      expect(result.error.message).toContain('503');
    }
  });

  it('maps HttpError (unmapped 4xx) to code "http_error" with the status in the message', () => {
    const result = mapErrorToToolResult(new HttpError(418, 'teapot'));

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('http_error');
      expect(result.error.message).toContain('418');
    }
  });

  it('maps MissingEtagError to code "missing_etag"', () => {
    const result = mapErrorToToolResult(
      new MissingEtagError('/v1/admin/event-types'),
    );

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('missing_etag');
    }
  });

  it('maps ZodError (response or local validation) to code "validation_error" with the first path', () => {
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

  it('maps a generic Error to code "internal_error" with the original message', () => {
    const result = mapErrorToToolResult(new Error('boom'));

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('internal_error');
      expect(result.error.message).toContain('boom');
    }
  });

  it('maps a non-Error throwable (string) to code "internal_error"', () => {
    const result = mapErrorToToolResult('string thrown');

    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe('internal_error');
      expect(result.error.message).toContain('string thrown');
    }
  });
});
