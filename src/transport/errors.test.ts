import { describe, it, expect } from 'vitest';
import {
  UnknownEventTypeError,
  EventValidationError,
  PermissionDeniedError,
  ServerError,
  TransportError,
  mapResponseError,
} from './errors.js';

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
  it('carries typeId, issues, and batchIndex', () => {
    const issues = [{ path: ['data', 'email'], message: 'required' }];
    const err = new EventValidationError('app.user.signed-up.v1', issues, 1);
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

  it('maps 403 to PermissionDeniedError', () => {
    const err = mapResponseError(403, { domain: 'app.user' });
    expect(err).toBeInstanceOf(PermissionDeniedError);
    expect((err as PermissionDeniedError).domain).toBe('app.user');
  });

  it('maps 5xx to ServerError carrying status and body', () => {
    const body = { code: 'INTERNAL' };
    const err = mapResponseError(503, body);
    expect(err).toBeInstanceOf(ServerError);
    expect((err as ServerError).status).toBe(503);
    expect((err as ServerError).body).toEqual(body);
  });

  it('falls back to generic TransportError for unrecognised 4xx', () => {
    const err = mapResponseError(418, { msg: 'teapot' });
    expect(err).toBeInstanceOf(TransportError);
    expect(err).not.toBeInstanceOf(UnknownEventTypeError);
    expect(err).not.toBeInstanceOf(EventValidationError);
    expect(err).not.toBeInstanceOf(PermissionDeniedError);
    expect(err).not.toBeInstanceOf(ServerError);
  });
});
