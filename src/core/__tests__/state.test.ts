import { describe, it, expect } from 'vitest';
import { computeState, computeOverallProgress, computeFeatureProgress } from '../state.js';
import type { ParseResult, ParsedEpic, ParsedFeature, FeatureState, StateSnapshot } from '../types.js';

function makeEpic(id = 'test-epic'): ParsedEpic {
  return {
    frontmatter: { id, type: 'epic', title: `Epic ${id}`, status: 'in_progress', created: '2026-04-05', updated: '2026-04-05' },
    description: '',
    goals: [],
    filePath: `.notickets/${id}/epic.md`,
  };
}

function makeFeature(id: string, epicId: string, tasks: ParsedFeature['tasks'] = [], phase: ParsedFeature['frontmatter']['phase'] = 'development'): ParsedFeature {
  return {
    frontmatter: { id, type: 'feature', epic: epicId, title: `Feature ${id}`, phase, status: 'in_progress', assignee: 'Claude', assignee_type: 'agent', created: '2026-04-05', updated: '2026-04-05' },
    description: '',
    tasks,
    acceptanceCriteria: [],
    filePath: `.notickets/${epicId}/${id}.md`,
  };
}

describe('computeState', () => {
  it('groups features under their parent epic', () => {
    const parsed: ParseResult = {
      epics: [makeEpic('onboarding')],
      features: [
        makeFeature('email', 'onboarding'),
        makeFeature('wizard', 'onboarding'),
      ],
    };

    const snapshot = computeState(parsed);

    expect(snapshot.epics).toHaveLength(1);
    expect(snapshot.epics[0]?.features).toHaveLength(2);
    expect(snapshot.epics[0]?.features[0]?.id).toBe('email');
    expect(snapshot.epics[0]?.features[1]?.id).toBe('wizard');
  });

  it('computes task totals from parsed tasks', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature('f1', 'test-epic', [
        { number: 1, title: 'Done', status: 'completed' },
        { number: 2, title: 'WIP', status: 'in_progress' },
        { number: 3, title: 'Todo', status: 'not_started' },
      ])],
    };

    const snapshot = computeState(parsed);
    const feature = snapshot.epics[0]?.features[0];

    expect(feature?.tasks.total).toBe(3);
    expect(feature?.tasks.completed).toBe(1);
  });

  it('maps frontmatter fields to feature state', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature('f1', 'test-epic', [], 'testing')],
    };

    const snapshot = computeState(parsed);
    const feature = snapshot.epics[0]?.features[0];

    expect(feature?.phase).toBe('testing');
    expect(feature?.assignee).toBe('Claude');
    expect(feature?.assigneeType).toBe('agent');
    expect(feature?.type).toBe('feature');
  });

  it('handles epics with no features', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [],
    };

    const snapshot = computeState(parsed);

    expect(snapshot.epics[0]?.features).toHaveLength(0);
  });

  it('handles features for multiple epics', () => {
    const parsed: ParseResult = {
      epics: [makeEpic('epic-a'), makeEpic('epic-b')],
      features: [
        makeFeature('f1', 'epic-a'),
        makeFeature('f2', 'epic-b'),
        makeFeature('f3', 'epic-a'),
      ],
    };

    const snapshot = computeState(parsed);

    expect(snapshot.epics[0]?.features).toHaveLength(2);
    expect(snapshot.epics[1]?.features).toHaveLength(1);
  });

  it('sets version to 1', () => {
    const snapshot = computeState({ epics: [], features: [] });
    expect(snapshot.version).toBe(1);
  });

  it('uses provided pushedAt timestamp', () => {
    const snapshot = computeState({ epics: [], features: [] }, '2026-04-05T12:00:00Z');
    expect(snapshot.pushedAt).toBe('2026-04-05T12:00:00Z');
  });

  it('generates pushedAt when not provided', () => {
    const snapshot = computeState({ epics: [], features: [] });
    expect(snapshot.pushedAt).toBeDefined();
    expect(new Date(snapshot.pushedAt).getTime()).not.toBeNaN();
  });

  it('initializes tests to zero', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature('f1', 'test-epic')],
    };

    const snapshot = computeState(parsed);
    const feature = snapshot.epics[0]?.features[0];

    expect(feature?.tests.total).toBe(0);
    expect(feature?.tests.passing).toBe(0);
  });

  it('drops features referencing non-existent epics', () => {
    const parsed: ParseResult = {
      epics: [makeEpic('real-epic')],
      features: [
        makeFeature('f1', 'real-epic'),
        makeFeature('f2', 'ghost-epic'),
      ],
    };

    const snapshot = computeState(parsed);

    expect(snapshot.epics).toHaveLength(1);
    expect(snapshot.epics[0]?.features).toHaveLength(1);
    expect(snapshot.epics[0]?.features[0]?.id).toBe('f1');
  });

  it('preserves meta from frontmatter', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [{
        ...makeFeature('f1', 'test-epic'),
        frontmatter: {
          ...makeFeature('f1', 'test-epic').frontmatter,
          meta: { quality_score: 82 },
        },
      }],
    };

    const snapshot = computeState(parsed);
    expect(snapshot.epics[0]?.features[0]?.meta).toEqual({ quality_score: 82 });
  });
});

describe('computeOverallProgress', () => {
  it('returns 0 for empty snapshot', () => {
    const snapshot: StateSnapshot = { version: 1, epics: [], pushedAt: '' };
    expect(computeOverallProgress(snapshot)).toBe(0);
  });

  it('returns 0 when no tasks exist', () => {
    const snapshot: StateSnapshot = {
      version: 1,
      epics: [{
        id: 'e', title: 'E', status: 'in_progress',
        features: [{ id: 'f', epicId: 'e', title: 'F', type: 'feature', phase: 'development', tasks: { total: 0, completed: 0 }, tests: { total: 0, passing: 0 } }],
      }],
      pushedAt: '',
    };
    expect(computeOverallProgress(snapshot)).toBe(0);
  });

  it('computes percentage across all features', () => {
    const snapshot: StateSnapshot = {
      version: 1,
      epics: [{
        id: 'e', title: 'E', status: 'in_progress',
        features: [
          { id: 'f1', epicId: 'e', title: 'F1', type: 'feature', phase: 'development', tasks: { total: 4, completed: 2 }, tests: { total: 0, passing: 0 } },
          { id: 'f2', epicId: 'e', title: 'F2', type: 'feature', phase: 'done', tasks: { total: 6, completed: 6 }, tests: { total: 0, passing: 0 } },
        ],
      }],
      pushedAt: '',
    };
    // 8/10 = 80%
    expect(computeOverallProgress(snapshot)).toBe(80);
  });

  it('clamps to 100 when completed exceeds total', () => {
    const snapshot: StateSnapshot = {
      version: 1,
      epics: [{
        id: 'e', title: 'E', status: 'in_progress',
        features: [
          { id: 'f1', epicId: 'e', title: 'F1', type: 'feature', phase: 'done', tasks: { total: 3, completed: 5 }, tests: { total: 0, passing: 0 } },
        ],
      }],
      pushedAt: '',
    };
    expect(computeOverallProgress(snapshot)).toBe(100);
  });

  it('rounds to nearest integer', () => {
    const snapshot: StateSnapshot = {
      version: 1,
      epics: [{
        id: 'e', title: 'E', status: 'in_progress',
        features: [
          { id: 'f1', epicId: 'e', title: 'F1', type: 'feature', phase: 'development', tasks: { total: 3, completed: 1 }, tests: { total: 0, passing: 0 } },
        ],
      }],
      pushedAt: '',
    };
    // 1/3 = 33.33... → 33
    expect(computeOverallProgress(snapshot)).toBe(33);
  });
});

describe('computeFeatureProgress', () => {
  function makeState(phase: FeatureState['phase'], tasks: FeatureState['tasks'], tests: FeatureState['tests']): FeatureState {
    return { id: 'f', epicId: 'e', title: 'F', type: 'feature', phase, tasks, tests };
  }

  it('returns 0 for ideation with no tasks and no tests', () => {
    expect(computeFeatureProgress(makeState('ideation', { total: 0, completed: 0 }, { total: 0, passing: 0 }))).toBe(0);
  });

  it('returns 100 for done with all tasks and tests complete', () => {
    expect(computeFeatureProgress(makeState('done', { total: 5, completed: 5 }, { total: 10, passing: 10 }))).toBe(100);
  });

  it('uses phase+task weighting when no tests exist', () => {
    // development=25, tasks=50% → 25*0.4 + 50*0.6 = 10 + 30 = 40
    const progress = computeFeatureProgress(makeState('development', { total: 4, completed: 2 }, { total: 0, passing: 0 }));
    expect(progress).toBe(40);
  });

  it('uses three-way weighting when tests exist', () => {
    // development=25, tasks 2/4=50%, tests 3/6=50%
    // 25*0.3 + 50*0.35 + 50*0.35 = 7.5 + 17.5 + 17.5 = 42.5 → 43
    const progress = computeFeatureProgress(makeState('development', { total: 4, completed: 2 }, { total: 6, passing: 3 }));
    expect(progress).toBe(43);
  });

  it('returns correct progress for review phase', () => {
    // review=75, tasks 3/3=100%, tests 5/5=100%
    // 75*0.3 + 100*0.35 + 100*0.35 = 22.5 + 35 + 35 = 92.5 → 93
    const progress = computeFeatureProgress(makeState('review', { total: 3, completed: 3 }, { total: 5, passing: 5 }));
    expect(progress).toBe(93);
  });

  it('uses two-way weighting when tests.total is zero', () => {
    // ideation=0, tasks 0/0=0%, tests.total=0 → two-way: 0*0.4 + 0*0.6 = 0
    const progress = computeFeatureProgress(makeState('ideation', { total: 0, completed: 0 }, { total: 0, passing: 0 }));
    expect(progress).toBe(0);
    // development=25, tasks 2/4=50%, tests.total=0 → two-way: 25*0.4 + 50*0.6 = 40
    const progress2 = computeFeatureProgress(makeState('development', { total: 4, completed: 2 }, { total: 0, passing: 0 }));
    expect(progress2).toBe(40);
    // Confirm three-way gives different result with same phase/tasks but tests.total > 0
    // development=25, tasks 2/4=50%, tests 0/1=0% → three-way: 25*0.3 + 50*0.35 + 0*0.35 = 25
    const progress3 = computeFeatureProgress(makeState('development', { total: 4, completed: 2 }, { total: 1, passing: 0 }));
    expect(progress3).toBe(25);
  });

  it('handles zero tasks with tests', () => {
    // testing=50, tasks 0/0=0%, tests 5/10=50%
    // 50*0.3 + 0*0.35 + 50*0.35 = 15 + 0 + 17.5 = 32.5 → 33
    const progress = computeFeatureProgress(makeState('testing', { total: 0, completed: 0 }, { total: 10, passing: 5 }));
    expect(progress).toBe(33);
  });
});

