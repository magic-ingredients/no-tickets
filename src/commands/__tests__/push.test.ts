import { describe, it, expect } from 'vitest';
import { assemblePush, mergeSession } from '../push.js';
import { pushSchema } from '../../core/schemas.js';
import type { FileEntry, Push, Session } from '../../core/types.js';

const testSession: Session = {
  agent: 'claude-code',
  agentType: 'agent',
  vendor: 'anthropic',
  environment: { os: 'darwin', runtime: 'v25.2.1', ci: false },
};

function epicFile(id: string): FileEntry {
  return {
    path: `.notickets/${id}/epic.md`,
    content: `---\nid: ${id}\ntype: epic\ntitle: Epic ${id}\nstatus: in_progress\ncreated: 2026-04-22\nupdated: 2026-04-22\n---\n# Epic ${id}\n`,
  };
}

function featureFile(id: string, epicId: string): FileEntry {
  return {
    path: `.notickets/${epicId}/${id}.md`,
    content: `---\nid: ${id}\ntype: feature\nepic: ${epicId}\ntitle: Feature ${id}\nphase: development\nstatus: in_progress\ncreated: 2026-04-22\nupdated: 2026-04-22\n---\n# Feature ${id}\n\n## Tasks\n\n### 1. First task\nstatus: not_started\n`,
  };
}

describe('assemblePush', () => {
  it('assembles a Push payload from files and session', () => {
    const files = [epicFile('auth'), featureFile('login', 'auth')];
    const result = assemblePush({
      files,
      projectId: 'proj-1',
      session: testSession,
      timestamp: '2026-04-22T10:00:00Z',
    });

    expect(result.projectId).toBe('proj-1');
    expect(result.timestamp).toBe('2026-04-22T10:00:00Z');
    expect(result.session).toEqual(testSession);
    expect(result.project?.entities).toBeDefined();
    expect(result.project?.entities.length).toBeGreaterThan(0);
  });

  it('includes epic, feature, and task entities from files', () => {
    const files = [epicFile('platform'), featureFile('api', 'platform')];
    const result = assemblePush({
      files,
      projectId: 'p1',
      session: testSession,
      timestamp: '2026-04-22T10:00:00Z',
    });

    const types = result.project?.entities.map((e) => e.type);
    expect(types).toContain('epic');
    expect(types).toContain('feature');
    expect(types).toContain('task');
  });

  it('omits project schema when no files provided', () => {
    const result = assemblePush({
      files: [],
      projectId: 'p1',
      session: testSession,
      timestamp: '2026-04-22T10:00:00Z',
    });

    expect(result.project).toBeUndefined();
  });

  it('generates timestamp when not provided', () => {
    const result = assemblePush({
      files: [],
      projectId: 'p1',
      session: testSession,
    });

    expect(result.timestamp).toBeDefined();
    expect(new Date(result.timestamp).getTime()).not.toBeNaN();
  });

  it('produces a payload that passes Zod validation', () => {
    const files = [epicFile('auth'), featureFile('login', 'auth')];
    const result = assemblePush({
      files,
      projectId: 'proj-1',
      session: testSession,
      timestamp: '2026-04-22T10:00:00Z',
    });

    expect(() => pushSchema.parse(result)).not.toThrow();
  });
});

describe('mergeSession', () => {
  it('adds session to a payload that has none', () => {
    const payload: Push = {
      projectId: 'p1',
      timestamp: '2026-04-22T10:00:00Z',
      quality: { score: 85, source: 'ci' },
    };

    const result = mergeSession(payload, testSession);

    expect(result.session).toEqual(testSession);
    expect(result.quality).toEqual({ score: 85, source: 'ci' });
  });

  it('does not overwrite existing session', () => {
    const existingSession: Session = {
      agent: 'cursor',
      agentType: 'agent',
      vendor: 'cursor',
    };
    const payload: Push = {
      projectId: 'p1',
      timestamp: '2026-04-22T10:00:00Z',
      session: existingSession,
    };

    const result = mergeSession(payload, testSession);

    expect(result.session).toEqual(existingSession);
  });

  it('preserves all other payload fields', () => {
    const payload: Push = {
      projectId: 'p1',
      timestamp: '2026-04-22T10:00:00Z',
      project: { entities: [{ id: 'e1', type: 'epic', title: 'E', status: 'not_started' }] },
      dev: { tasks: [{ entityId: 'e1', phase: 'red' }] },
      custom: { myData: true },
    };

    const result = mergeSession(payload, testSession);

    expect(result.project).toEqual(payload.project);
    expect(result.dev).toEqual(payload.dev);
    expect(result.custom).toEqual(payload.custom);
  });
});
