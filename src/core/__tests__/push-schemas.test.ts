import { describe, it, expect } from 'vitest';
import {
  pushSchema,
  sessionSchema,
  pushEnvironmentSchema,
  projectEntityTypeSchema,
  devPhaseSchema,
  acceptanceStatusSchema,
  prioritySchema,
  qualitySourceSchema,
  projectEntitySchema,
  projectDataSchema,
  devReviewSchema,
  devTaskSchema,
  devDataSchema,
  pmUpdateSchema,
  pmDataSchema,
  qualityDataSchema,
} from '../schemas.js';

// -- Enum schemas -------------------------------------------------------------

describe('projectEntityTypeSchema', () => {
  it('accepts valid entity types', () => {
    expect(projectEntityTypeSchema.parse('epic')).toBe('epic');
    expect(projectEntityTypeSchema.parse('feature')).toBe('feature');
    expect(projectEntityTypeSchema.parse('task')).toBe('task');
  });

  it('rejects invalid entity type', () => {
    expect(() => projectEntityTypeSchema.parse('story')).toThrow();
  });
});

describe('devPhaseSchema', () => {
  it('accepts valid phases', () => {
    for (const phase of ['red', 'green', 'refactor', 'review', 'complete']) {
      expect(devPhaseSchema.parse(phase)).toBe(phase);
    }
  });

  it('rejects invalid phase', () => {
    expect(() => devPhaseSchema.parse('testing')).toThrow();
  });
});

describe('acceptanceStatusSchema', () => {
  it('accepts valid statuses', () => {
    for (const s of ['unreviewed', 'accepted', 'changes_requested']) {
      expect(acceptanceStatusSchema.parse(s)).toBe(s);
    }
  });

  it('rejects invalid status', () => {
    expect(() => acceptanceStatusSchema.parse('approved')).toThrow();
  });
});

describe('prioritySchema', () => {
  it('accepts valid priorities', () => {
    for (const p of ['critical', 'high', 'medium', 'low']) {
      expect(prioritySchema.parse(p)).toBe(p);
    }
  });

  it('rejects invalid priority', () => {
    expect(() => prioritySchema.parse('urgent')).toThrow();
  });
});

describe('qualitySourceSchema', () => {
  it('accepts valid sources', () => {
    expect(qualitySourceSchema.parse('local')).toBe('local');
    expect(qualitySourceSchema.parse('ci')).toBe('ci');
  });

  it('rejects invalid source', () => {
    expect(() => qualitySourceSchema.parse('staging')).toThrow();
  });
});

// -- Core envelope schemas ----------------------------------------------------

describe('pushEnvironmentSchema', () => {
  it('accepts full environment', () => {
    const env = { os: 'darwin', runtime: 'v25.2.1', ci: true, ciProvider: 'github-actions' };
    expect(pushEnvironmentSchema.parse(env)).toEqual(env);
  });

  it('accepts empty object (all optional)', () => {
    expect(pushEnvironmentSchema.parse({})).toEqual({});
  });

  it('rejects ci as string', () => {
    expect(() => pushEnvironmentSchema.parse({ ci: 'true' })).toThrow();
  });
});

describe('sessionSchema', () => {
  it('accepts full session', () => {
    const session = {
      agent: 'claude-code',
      agentType: 'agent',
      model: 'claude-opus-4',
      vendor: 'anthropic',
      environment: { os: 'darwin', runtime: 'v25.2.1', ci: false },
      duration: 120,
      result: 'success',
      meta: { customField: 42 },
    };
    expect(sessionSchema.parse(session)).toEqual(session);
  });

  it('accepts minimal session (agent + agentType only)', () => {
    const session = { agent: 'unknown', agentType: 'human' };
    expect(sessionSchema.parse(session)).toEqual(session);
  });

  it('rejects missing agent', () => {
    expect(() => sessionSchema.parse({ agentType: 'human' })).toThrow();
  });

  it('rejects invalid agentType', () => {
    expect(() => sessionSchema.parse({ agent: 'x', agentType: 'bot' })).toThrow();
  });

  it('rejects negative duration', () => {
    expect(() => sessionSchema.parse({ agent: 'x', agentType: 'human', duration: -1 })).toThrow();
  });

  it('preserves meta passthrough', () => {
    const session = { agent: 'x', agentType: 'agent', meta: { nested: { deep: true } } };
    const parsed = sessionSchema.parse(session);
    expect(parsed.meta).toEqual({ nested: { deep: true } });
  });
});

// -- Project schema -----------------------------------------------------------

describe('projectEntitySchema', () => {
  it('accepts full entity', () => {
    const entity = {
      id: 'feat-1',
      type: 'feature',
      parentId: 'epic-1',
      title: 'Auth flow',
      status: 'in_progress',
      assignee: 'alice',
      assigneeType: 'human',
      meta: { priority: 'high' },
    };
    expect(projectEntitySchema.parse(entity)).toEqual(entity);
  });

  it('accepts minimal entity (required fields only)', () => {
    const entity = { id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' };
    expect(projectEntitySchema.parse(entity)).toEqual(entity);
  });

  it('rejects missing id', () => {
    expect(() => projectEntitySchema.parse({ type: 'epic', title: 'X', status: 'not_started' })).toThrow();
  });

  it('rejects invalid type', () => {
    expect(() => projectEntitySchema.parse({ id: 'x', type: 'story', title: 'X', status: 'not_started' })).toThrow();
  });

  it('rejects invalid status', () => {
    expect(() => projectEntitySchema.parse({ id: 'x', type: 'epic', title: 'X', status: 'done' })).toThrow();
  });
});

describe('projectDataSchema', () => {
  it('accepts entities array', () => {
    const data = { entities: [{ id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' }] };
    expect(projectDataSchema.parse(data)).toEqual(data);
  });

  it('accepts empty entities array', () => {
    expect(projectDataSchema.parse({ entities: [] })).toEqual({ entities: [] });
  });

  it('rejects missing entities', () => {
    expect(() => projectDataSchema.parse({})).toThrow();
  });
});

// -- Dev schema ---------------------------------------------------------------

describe('devReviewSchema', () => {
  it('accepts full review', () => {
    const review = { reviewer: 'adversarial', verdict: 'needs-refactoring', findings: 3 };
    expect(devReviewSchema.parse(review)).toEqual(review);
  });

  it('accepts minimal review', () => {
    const review = { reviewer: 'mutation', verdict: 'clean' };
    expect(devReviewSchema.parse(review)).toEqual(review);
  });

  it('rejects missing reviewer', () => {
    expect(() => devReviewSchema.parse({ verdict: 'clean' })).toThrow();
  });

  it('rejects missing verdict', () => {
    expect(() => devReviewSchema.parse({ reviewer: 'adversarial' })).toThrow();
  });
});

describe('devTaskSchema', () => {
  it('accepts full task', () => {
    const task = {
      entityId: 'feat-1',
      phase: 'green',
      commitSha: 'abc1234',
      startedAt: '2026-04-22T10:00:00Z',
      completedAt: '2026-04-22T10:30:00Z',
      duration: 1800,
      reviews: [{ reviewer: 'adversarial', verdict: 'clean' }],
      meta: { model: 'claude-opus-4' },
    };
    expect(devTaskSchema.parse(task)).toEqual(task);
  });

  it('accepts minimal task (entityId only)', () => {
    expect(devTaskSchema.parse({ entityId: 'feat-1' })).toEqual({ entityId: 'feat-1' });
  });

  it('rejects missing entityId', () => {
    expect(() => devTaskSchema.parse({ phase: 'red' })).toThrow();
  });

  it('rejects invalid phase', () => {
    expect(() => devTaskSchema.parse({ entityId: 'x', phase: 'testing' })).toThrow();
  });

  it('rejects negative duration', () => {
    expect(() => devTaskSchema.parse({ entityId: 'x', duration: -100 })).toThrow();
  });
});

describe('devDataSchema', () => {
  it('accepts full dev data', () => {
    const data = {
      tasks: [{ entityId: 'feat-1', phase: 'red' }],
      meta: { sessionId: 'abc' },
    };
    expect(devDataSchema.parse(data)).toEqual(data);
  });

  it('accepts empty object (all optional)', () => {
    expect(devDataSchema.parse({})).toEqual({});
  });
});

// -- PM schema ----------------------------------------------------------------

describe('pmUpdateSchema', () => {
  it('accepts full update', () => {
    const update = {
      entityId: 'feat-1',
      acceptance: 'accepted',
      priority: 'high',
      labels: ['mvp', 'launch'],
      releaseId: 'v2.0',
      notes: 'Ready for review',
      meta: { reviewer: 'alice' },
    };
    expect(pmUpdateSchema.parse(update)).toEqual(update);
  });

  it('accepts minimal update (entityId only)', () => {
    expect(pmUpdateSchema.parse({ entityId: 'feat-1' })).toEqual({ entityId: 'feat-1' });
  });

  it('rejects invalid acceptance status', () => {
    expect(() => pmUpdateSchema.parse({ entityId: 'x', acceptance: 'approved' })).toThrow();
  });

  it('rejects invalid priority', () => {
    expect(() => pmUpdateSchema.parse({ entityId: 'x', priority: 'urgent' })).toThrow();
  });
});

describe('pmDataSchema', () => {
  it('accepts updates array', () => {
    const data = { updates: [{ entityId: 'feat-1', acceptance: 'unreviewed' }], meta: {} };
    expect(pmDataSchema.parse(data)).toEqual(data);
  });

  it('rejects missing updates', () => {
    expect(() => pmDataSchema.parse({})).toThrow();
  });
});

// -- Quality schema -----------------------------------------------------------

describe('qualityDataSchema', () => {
  it('accepts full quality data', () => {
    const data = {
      score: 85,
      grade: 'B+',
      source: 'ci',
      entityId: 'feat-1',
      categories: { security: 90, maintainability: 80 },
      meta: { tool: 'sonarqube' },
    };
    expect(qualityDataSchema.parse(data)).toEqual(data);
  });

  it('accepts minimal quality data (score only)', () => {
    expect(qualityDataSchema.parse({ score: 100 })).toEqual({ score: 100 });
  });

  it('rejects missing score', () => {
    expect(() => qualityDataSchema.parse({ grade: 'A' })).toThrow();
  });

  it('rejects invalid source', () => {
    expect(() => qualityDataSchema.parse({ score: 50, source: 'staging' })).toThrow();
  });

  it('rejects non-numeric score', () => {
    expect(() => qualityDataSchema.parse({ score: 'high' })).toThrow();
  });

  it('rejects negative score', () => {
    expect(() => qualityDataSchema.parse({ score: -1 })).toThrow();
  });

  it('rejects Infinity score', () => {
    expect(() => qualityDataSchema.parse({ score: Infinity })).toThrow();
  });

  it('rejects NaN score', () => {
    expect(() => qualityDataSchema.parse({ score: NaN })).toThrow();
  });
});

// -- Push envelope ------------------------------------------------------------

describe('pushSchema', () => {
  it('accepts full push payload', () => {
    const push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      session: { agent: 'claude-code', agentType: 'agent' },
      project: { entities: [{ id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' }] },
      dev: { tasks: [{ entityId: 'e-1', phase: 'red' }] },
      pm: { updates: [{ entityId: 'e-1', acceptance: 'unreviewed' }] },
      quality: { score: 85, source: 'local' },
      custom: { myTool: { data: 123 } },
    };
    expect(pushSchema.parse(push)).toEqual(push);
  });

  it('accepts minimal push (projectId + timestamp only)', () => {
    const push = { projectId: 'proj-1', timestamp: '2026-04-22T10:00:00Z' };
    expect(pushSchema.parse(push)).toEqual(push);
  });

  it('rejects missing projectId', () => {
    expect(() => pushSchema.parse({ timestamp: '2026-04-22T10:00:00Z' })).toThrow();
  });

  it('rejects missing timestamp', () => {
    expect(() => pushSchema.parse({ projectId: 'proj-1' })).toThrow();
  });

  it('accepts push with only one schema populated', () => {
    const push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      quality: { score: 92, source: 'ci' },
    };
    expect(pushSchema.parse(push)).toEqual(push);
  });

  it('rejects invalid nested schema data', () => {
    const push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      quality: { score: 'high' },
    };
    expect(() => pushSchema.parse(push)).toThrow();
  });

  it('preserves custom passthrough data', () => {
    const push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      custom: { myFramework: { events: [1, 2, 3], nested: { deep: true } } },
    };
    const parsed = pushSchema.parse(push);
    expect(parsed.custom).toEqual({ myFramework: { events: [1, 2, 3], nested: { deep: true } } });
  });
});
