export class TransportError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'TransportError';
  }
}

export class MissingEtagError extends TransportError {
  constructor(path: string) {
    super(`registry response from ${path} is missing the ETag header`);
    this.name = 'MissingEtagError';
  }
}

export class HttpError extends TransportError {
  readonly status: number;
  readonly body: unknown;

  constructor(status: number, body: unknown) {
    super(`request failed with status ${status}`);
    this.name = 'HttpError';
    this.status = status;
    this.body = body;
  }
}

export class UnknownEventTypeError extends TransportError {
  readonly typeId: string;
  readonly batchIndex: number;

  constructor(typeId: string, batchIndex: number) {
    super(`unknown event type "${typeId}" at batch index ${batchIndex}`);
    this.name = 'UnknownEventTypeError';
    this.typeId = typeId;
    this.batchIndex = batchIndex;
  }
}

export interface ValidationIssue {
  readonly path: readonly (string | number)[];
  readonly message: string;
}

export class EventValidationError extends TransportError {
  readonly typeId: string;
  readonly batchIndex: number;
  readonly issues: readonly ValidationIssue[];

  constructor(typeId: string, batchIndex: number, issues: readonly ValidationIssue[]) {
    super(`event "${typeId}" failed validation at batch index ${batchIndex}`);
    this.name = 'EventValidationError';
    this.typeId = typeId;
    this.batchIndex = batchIndex;
    this.issues = issues;
  }
}

export class PermissionDeniedError extends TransportError {
  readonly domain: string;

  constructor(domain: string) {
    super(`permission denied for domain "${domain}"`);
    this.name = 'PermissionDeniedError';
    this.domain = domain;
  }
}

export class ServerError extends TransportError {
  readonly status: number;
  readonly body: unknown;

  constructor(status: number, body: unknown) {
    super(`server returned ${status}`);
    this.name = 'ServerError';
    this.status = status;
    this.body = body;
  }
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null;
}

function asString(v: unknown): string | undefined {
  return typeof v === 'string' && v.length > 0 ? v : undefined;
}

function asNumber(v: unknown): number | undefined {
  return typeof v === 'number' ? v : undefined;
}

function asIssues(v: unknown): readonly ValidationIssue[] {
  if (!Array.isArray(v)) return [];
  return v.flatMap((entry): ValidationIssue[] => {
    if (!isRecord(entry)) return [];
    const message = asString(entry['message']);
    if (message === undefined) return [];
    const rawPath = entry['path'];
    const path = Array.isArray(rawPath)
      ? rawPath.filter((p): p is string | number => typeof p === 'string' || typeof p === 'number')
      : [];
    return [{ path, message }];
  });
}

export function mapResponseError(status: number, body: unknown): TransportError {
  if (status === 422 && isRecord(body)) {
    const code = asString(body['code']);
    const typeId = asString(body['typeId']);
    const batchIndex = asNumber(body['batchIndex']);
    if (code === 'unknown_event_type' && typeId !== undefined && batchIndex !== undefined) {
      return new UnknownEventTypeError(typeId, batchIndex);
    }
    if (code === 'event_validation' && typeId !== undefined && batchIndex !== undefined) {
      return new EventValidationError(typeId, batchIndex, asIssues(body['issues']));
    }
  }

  if (status === 403 && isRecord(body)) {
    const domain = asString(body['domain']);
    if (domain !== undefined) {
      return new PermissionDeniedError(domain);
    }
  }

  if (status >= 500) {
    return new ServerError(status, body);
  }

  return new HttpError(status, body);
}
