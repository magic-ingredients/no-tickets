// Three subject runners (create / get / list) live in this single file
// rather than separate files. Each is a thin wrapper over the transport
// facade with little logic, and grouping them keeps shared types
// (SubjectDeps, exit codes) close to their consumers. The test file mirrors
// this layout. The PRD's three-file split was a planning hint; the
// behaviour is what matters.

import type { Subject, SubjectRef } from '../../../core/subject.js';
import type { SubjectListQuery } from '../../../transport/subjects.js';
import { HttpError } from '../../../transport/errors.js';

export interface SubjectDeps {
  create(subject: Subject): Promise<Subject>;
  get(ref: SubjectRef): Promise<Subject>;
  list(query: SubjectListQuery): Promise<readonly Subject[]>;
  write(line: string): void;
  writeErr(line: string): void;
}

const EXIT_OK = 0;
const EXIT_VALIDATION = 1;
const EXIT_SERVER = 3;

export interface SubjectCreateOptions {
  readonly type: string;
  readonly externalId: string;
  readonly displayName: string;
  readonly metadata?: string;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

export async function runSubjectCreate(
  options: SubjectCreateOptions,
  deps: SubjectDeps,
): Promise<number> {
  let metadata: Record<string, unknown> | undefined;
  if (options.metadata !== undefined) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(options.metadata);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      deps.writeErr(`--metadata is not valid JSON: ${message}`);
      return EXIT_VALIDATION;
    }
    const record = asRecord(parsed);
    if (record === null) {
      deps.writeErr('--metadata must be a JSON object');
      return EXIT_VALIDATION;
    }
    metadata = record;
  }

  const subject: Subject = {
    type: options.type,
    externalId: options.externalId,
    displayName: options.displayName,
    ...(metadata !== undefined && { metadata }),
  };

  let created: Subject;
  try {
    created = await deps.create(subject);
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_SERVER;
  }

  deps.write(JSON.stringify(created, null, 2));
  return EXIT_OK;
}

export async function runSubjectGet(
  ref: SubjectRef,
  deps: SubjectDeps,
): Promise<number> {
  if (ref.type.length === 0) {
    deps.writeErr('subject get: --type is required');
    return EXIT_VALIDATION;
  }
  if (ref.id.length === 0) {
    deps.writeErr('subject get: --id is required');
    return EXIT_VALIDATION;
  }

  let subject: Subject;
  try {
    subject = await deps.get(ref);
  } catch (err) {
    if (err instanceof HttpError && err.status === 404) {
      deps.writeErr(`subject not found: ${ref.type}/${ref.id}`);
      return EXIT_VALIDATION;
    }
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_SERVER;
  }

  deps.write(JSON.stringify(subject, null, 2));
  return EXIT_OK;
}

export interface SubjectListOptions {
  readonly type: string;
  readonly format?: 'json' | 'table';
}

function renderTable(subjects: readonly Subject[]): string[] {
  if (subjects.length === 0) return ['No subjects.'];
  const header = ['externalId', 'displayName'];
  const rows = subjects.map((s) => [s.externalId, s.displayName]);
  const widths = header.map((h, i) =>
    Math.max(h.length, ...rows.map((r) => (r[i] ?? '').length)),
  );
  const fmt = (cells: string[]): string =>
    cells.map((c, i) => c.padEnd(widths[i] ?? 0)).join('  ');
  return [fmt(header), ...rows.map(fmt)];
}

export async function runSubjectList(
  options: SubjectListOptions,
  deps: SubjectDeps,
): Promise<number> {
  if (options.type.length === 0) {
    deps.writeErr('--type is required');
    return EXIT_VALIDATION;
  }
  let subjects: readonly Subject[];
  try {
    subjects = await deps.list({ type: options.type });
  } catch (err) {
    deps.writeErr(err instanceof Error ? err.message : String(err));
    return EXIT_SERVER;
  }

  if (options.format === 'table') {
    for (const line of renderTable(subjects)) deps.write(line);
  } else {
    deps.write(JSON.stringify(subjects, null, 2));
  }
  return EXIT_OK;
}
