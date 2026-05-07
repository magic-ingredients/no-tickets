import { describe, it, expect } from 'vitest';
import {
  TransportError,
  HttpError,
  UnknownEventTypeError,
  EventValidationError,
  PermissionDeniedError,
  ServerError,
  mapResponseError,
} from './errors.js';

describe('HttpError', () => {
  it('preserves status and body for unrecognised HTTP errors', () => {
    const body = { msg: 'teapot' };
    const err = new HttpError(418, body);
    expect(err).toBeInstanceOf(TransportError);
    expect(err.status).toBe(418);
    expect(err.body).toEqual(body);
    expect(err.name).toBe('HttpError');
  });
});

describe('UnknownEventTypeError', () => {
  it('carries typeId and batchIndex', () => {
    const err = new UnknownEventTypeError('app.user.signed-up.v1', 2);
    expect(err).toBeInstanceOf(Error);
    expect(err).toBeInstanceOf(TransportError);
    expect(err.typeId).toBe('app.user.signed-up.v1');
    expect(err.batchIndex).toBe(2);
    expect(err.name).toBe('UnknownEventTypeError');
  });

  it('includes typeId and batchIndex in the message', () => {
    const err = new UnknownEventTypeError('app.user.signed-up.v1', 0);
    expect(err.message).toContain('app.user.signed-up.v1');
    expect(err.message).toContain('0');
  });
});

describe('EventValidationError', () => {
  it('carries typeId, batchIndex, and issues (positional order matches sibling errors)', () => {
    const issues = [{ path: ['data', 'email'], message: 'required' }];
    const err = new EventValidationError('app.user.signed-up.v1', 1, issues);
    expect(err).toBeInstanceOf(TransportError);
    expect(err.typeId).toBe('app.user.signed-up.v1');
    expect(err.batchIndex).toBe(1);
    expect(err.issues).toEqual(issues);
    expect(err.name).toBe('EventValidationError');
  });
});

describe('PermissionDeniedError', () => {
  it('carries the failing domain', () => {
    const err = new PermissionDeniedError('app.user');
    expect(err).toBeInstanceOf(TransportError);
    expect(err.domain).toBe('app.user');
    expect(err.name).toBe('PermissionDeniedError');
    expect(err.message).toContain('app.user');
  });
});

describe('ServerError', () => {
  it('carries status and body', () => {
    const err = new ServerError(503, 'service unavailable');
    expect(err).toBeInstanceOf(TransportError);
    expect(err.status).toBe(503);
    expect(err.body).toBe('service unavailable');
    expect(err.name).toBe('ServerError');
  });

  it('accepts an object body', () => {
    const body = { code: 'INTERNAL', message: 'oops' };
    const err = new ServerError(500, body);
    expect(err.body).toEqual(body);
  });
});

describe('mapResponseError', () => {
  it('maps 422 unknown_event_type to UnknownEventTypeError', () => {
    const err = mapResponseError(422, {
      code: 'unknown_event_type',
      typeId: 'app.user.signed-up.v1',
      batchIndex: 1,
    });
    expect(err).toBeInstanceOf(UnknownEventTypeError);
    expect((err as UnknownEventTypeError).typeId).toBe('app.user.signed-up.v1');
    expect((err as UnknownEventTypeError).batchIndex).toBe(1);
  });

  it('maps 422 event_validation to EventValidationError', () => {
    const issues = [{ path: ['data', 'email'], message: 'required' }];
    const err = mapResponseError(422, {
      code: 'event_validation',
      typeId: 'app.user.signed-up.v1',
      batchIndex: 0,
      issues,
    });
    expect(err).toBeInstanceOf(EventValidationError);
    expect((err as EventValidationError).issues).toEqual(issues);
    expect((err as EventValidationError).batchIndex).toBe(0);
  });

  it('falls through to HttpError on malformed 422 (missing typeId)', () => {
    const body = { code: 'unknown_event_type', batchIndex: 0 };
    const err = mapResponseError(422, body);
    expect(err).toBeInstanceOf(HttpError);
    expect(err).not.toBeInstanceOf(UnknownEventTypeError);
    expect((err as HttpError).status).toBe(422);
    expect((err as HttpError).body).toEqual(body);
  });

  it('falls through to HttpError on malformed 422 (missing batchIndex)', () => {
    const body = { code: 'event_validation', typeId: 'app.x.v1', issues: [] };
    const err = mapResponseError(422, body);
    expect(err).toBeInstanceOf(HttpError);
    expect(err).not.toBeInstanceOf(EventValidationError);
    expect((err as HttpError).body).toEqual(body);
  });

  it('falls through to HttpError on 422 with non-record body', () => {
    const err = mapResponseError(422, 'unprocessable');
    expect(err).toBeInstanceOf(HttpError);
    expect((err as HttpError).status).toBe(422);
    expect((err as HttpError).body).toBe('unprocessable');
  });

  it('maps 403 with domain to PermissionDeniedError', () => {
    const err = mapResponseError(403, { domain: 'app.user' });
    expect(err).toBeInstanceOf(PermissionDeniedError);
    expect((err as PermissionDeniedError).domain).toBe('app.user');
  });

  it('falls through to HttpError on 403 without a domain', () => {
    const err = mapResponseError(403, { msg: 'forbidden' });
    expect(err).toBeInstanceOf(HttpError);
    expect(err).not.toBeInstanceOf(PermissionDeniedError);
    expect((err as HttpError).status).toBe(403);
  });

  it('maps 5xx to ServerError carrying status and body', () => {
    const body = { code: 'INTERNAL' };
    const err = mapResponseError(503, body);
    expect(err).toBeInstanceOf(ServerError);
    expect((err as ServerError).status).toBe(503);
    expect((err as ServerError).body).toEqual(body);
  });

  it('maps unrecognised 4xx to HttpError preserving status and body', () => {
    const body = { msg: 'teapot' };
    const err = mapResponseError(418, body);
    expect(err).toBeInstanceOf(HttpError);
    expect(err).not.toBeInstanceOf(UnknownEventTypeError);
    expect(err).not.toBeInstanceOf(EventValidationError);
    expect(err).not.toBeInstanceOf(PermissionDeniedError);
    expect(err).not.toBeInstanceOf(ServerError);
    expect((err as HttpError).status).toBe(418);
    expect((err as HttpError).body).toEqual(body);
  });
});
