import { describe, it, expect } from 'vitest';
import {
  pushSchema,
  sessionSchema,
  pushEnvironmentSchema,
  workEntityTypeSchema,
  engineeringPhaseSchema,
  acceptanceStatusSchema,
  prioritySchema,
  codeQualitySourceSchema,
  workEntitySchema,
  workDataSchema,
  engineeringReviewSchema,
  engineeringTaskSchema,
  engineeringDataSchema,
  productUpdateSchema,
  productDataSchema,
  codeQualityDataSchema,
} from '../schemas.js';

// -- Enum schemas -------------------------------------------------------------

describe('workEntityTypeSchema', () => {
  it('accepts valid entity types', () => {
    expect(workEntityTypeSchema.parse('epic')).toBe('epic');
    expect(workEntityTypeSchema.parse('feature')).toBe('feature');
    expect(workEntityTypeSchema.parse('task')).toBe('task');
  });

  it('rejects invalid entity type', () => {
    expect(() => workEntityTypeSchema.parse('story')).toThrow();
  });
});

describe('engineeringPhaseSchema', () => {
  it('accepts valid phases', () => {
    for (const phase of ['red', 'green', 'refactor', 'review', 'complete']) {
      expect(engineeringPhaseSchema.parse(phase)).toBe(phase);
    }
  });

  it('rejects invalid phase', () => {
    expect(() => engineeringPhaseSchema.parse('testing')).toThrow();
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

describe('codeQualitySourceSchema', () => {
  it('accepts valid sources', () => {
    expect(codeQualitySourceSchema.parse('local')).toBe('local');
    expect(codeQualitySourceSchema.parse('ci')).toBe('ci');
  });

  it('rejects invalid source', () => {
    expect(() => codeQualitySourceSchema.parse('staging')).toThrow();
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

// -- Work schema --------------------------------------------------------------

describe('workEntitySchema', () => {
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
    expect(workEntitySchema.parse(entity)).toEqual(entity);
  });

  it('accepts minimal entity (required fields only)', () => {
    const entity = { id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' };
    expect(workEntitySchema.parse(entity)).toEqual(entity);
  });

  it('rejects missing id', () => {
    expect(() => workEntitySchema.parse({ type: 'epic', title: 'X', status: 'not_started' })).toThrow();
  });

  it('rejects invalid type', () => {
    expect(() => workEntitySchema.parse({ id: 'x', type: 'story', title: 'X', status: 'not_started' })).toThrow();
  });

  it('rejects invalid status', () => {
    expect(() => workEntitySchema.parse({ id: 'x', type: 'epic', title: 'X', status: 'done' })).toThrow();
  });
});

describe('workDataSchema', () => {
  it('accepts entities array', () => {
    const data = { entities: [{ id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' }] };
    expect(workDataSchema.parse(data)).toEqual(data);
  });

  it('accepts empty entities array', () => {
    expect(workDataSchema.parse({ entities: [] })).toEqual({ entities: [] });
  });

  it('rejects missing entities', () => {
    expect(() => workDataSchema.parse({})).toThrow();
  });
});

// -- Engineering schema -------------------------------------------------------

describe('engineeringReviewSchema', () => {
  it('accepts full review', () => {
    const review = { reviewer: 'adversarial', verdict: 'needs-refactoring', findings: 3 };
    expect(engineeringReviewSchema.parse(review)).toEqual(review);
  });

  it('accepts minimal review', () => {
    const review = { reviewer: 'mutation', verdict: 'clean' };
    expect(engineeringReviewSchema.parse(review)).toEqual(review);
  });

  it('rejects missing reviewer', () => {
    expect(() => engineeringReviewSchema.parse({ verdict: 'clean' })).toThrow();
  });

  it('rejects missing verdict', () => {
    expect(() => engineeringReviewSchema.parse({ reviewer: 'adversarial' })).toThrow();
  });
});

describe('engineeringTaskSchema', () => {
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
    expect(engineeringTaskSchema.parse(task)).toEqual(task);
  });

  it('accepts minimal task (entityId only)', () => {
    expect(engineeringTaskSchema.parse({ entityId: 'feat-1' })).toEqual({ entityId: 'feat-1' });
  });

  it('rejects missing entityId', () => {
    expect(() => engineeringTaskSchema.parse({ phase: 'red' })).toThrow();
  });

  it('rejects invalid phase', () => {
    expect(() => engineeringTaskSchema.parse({ entityId: 'x', phase: 'testing' })).toThrow();
  });

  it('rejects negative duration', () => {
    expect(() => engineeringTaskSchema.parse({ entityId: 'x', duration: -100 })).toThrow();
  });
});

describe('engineeringDataSchema', () => {
  it('accepts full dev data', () => {
    const data = {
      tasks: [{ entityId: 'feat-1', phase: 'red' }],
      meta: { sessionId: 'abc' },
    };
    expect(engineeringDataSchema.parse(data)).toEqual(data);
  });

  it('accepts empty object (all optional)', () => {
    expect(engineeringDataSchema.parse({})).toEqual({});
  });
});

// -- Product schema -----------------------------------------------------------

describe('productUpdateSchema', () => {
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
    expect(productUpdateSchema.parse(update)).toEqual(update);
  });

  it('accepts minimal update (entityId only)', () => {
    expect(productUpdateSchema.parse({ entityId: 'feat-1' })).toEqual({ entityId: 'feat-1' });
  });

  it('rejects invalid acceptance status', () => {
    expect(() => productUpdateSchema.parse({ entityId: 'x', acceptance: 'approved' })).toThrow();
  });

  it('rejects invalid priority', () => {
    expect(() => productUpdateSchema.parse({ entityId: 'x', priority: 'urgent' })).toThrow();
  });
});

describe('productDataSchema', () => {
  it('accepts updates array', () => {
    const data = { updates: [{ entityId: 'feat-1', acceptance: 'unreviewed' }], meta: {} };
    expect(productDataSchema.parse(data)).toEqual(data);
  });

  it('rejects missing updates', () => {
    expect(() => productDataSchema.parse({})).toThrow();
  });
});

// -- Code quality schema ------------------------------------------------------

describe('codeQualityDataSchema', () => {
  it('accepts full quality data', () => {
    const data = {
      score: 85,
      grade: 'B+',
      source: 'ci',
      entityId: 'feat-1',
      categories: { security: 90, maintainability: 80 },
      meta: { tool: 'sonarqube' },
    };
    expect(codeQualityDataSchema.parse(data)).toEqual(data);
  });

  it('accepts minimal quality data (score only)', () => {
    expect(codeQualityDataSchema.parse({ score: 100 })).toEqual({ score: 100 });
  });

  it('rejects missing score', () => {
    expect(() => codeQualityDataSchema.parse({ grade: 'A' })).toThrow();
  });

  it('rejects invalid source', () => {
    expect(() => codeQualityDataSchema.parse({ score: 50, source: 'staging' })).toThrow();
  });

  it('rejects non-numeric score', () => {
    expect(() => codeQualityDataSchema.parse({ score: 'high' })).toThrow();
  });

  it('rejects negative score', () => {
    expect(() => codeQualityDataSchema.parse({ score: -1 })).toThrow();
  });

  it('rejects Infinity score', () => {
    expect(() => codeQualityDataSchema.parse({ score: Infinity })).toThrow();
  });

  it('rejects NaN score', () => {
    expect(() => codeQualityDataSchema.parse({ score: NaN })).toThrow();
  });
});

// -- Push envelope ------------------------------------------------------------

describe('pushSchema', () => {
  it('accepts full push payload', () => {
    const push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      session: { agent: 'claude-code', agentType: 'agent' },
      work: { entities: [{ id: 'e-1', type: 'epic', title: 'Platform', status: 'not_started' }] },
      engineering: { tasks: [{ entityId: 'e-1', phase: 'red' }] },
      product: { updates: [{ entityId: 'e-1', acceptance: 'unreviewed' }] },
      codeQuality: { score: 85, source: 'local' },
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
      codeQuality: { score: 92, source: 'ci' },
    };
    expect(pushSchema.parse(push)).toEqual(push);
  });

  it('rejects invalid nested schema data', () => {
    const push = {
      projectId: 'proj-1',
      timestamp: '2026-04-22T10:00:00Z',
      codeQuality: { score: 'high' },
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
