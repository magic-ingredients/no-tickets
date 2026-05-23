import { describe, it, expect } from 'vitest';
import { computeDiff } from '../diff.js';
import type { StateSnapshot, FeatureState } from '../types.js';

function makeFeatureState(id: string, overrides: Partial<FeatureState> = {}): FeatureState {
  return {
    id,
    epicId: 'test-epic',
    title: `Feature ${id}`,
    type: 'feature',
    phase: 'development',
    tasks: { total: 3, completed: 1 },
    tests: { total: 5, passing: 3 },
    ...overrides,
  };
}

function makeSnapshot(features: readonly FeatureState[]): StateSnapshot {
  return {
    version: 1,
    epics: [{ id: 'test-epic', title: 'Test', status: 'in_progress', features }],
    computedAt: '2026-04-05T00:00:00Z',
  };
}

describe('computeDiff', () => {
  it('treats all features as added when no previous snapshot', () => {
    const current = makeSnapshot([makeFeatureState('f1'), makeFeatureState('f2')]);

    const delta = computeDiff(undefined, current);

    expect(delta.added).toHaveLength(2);
    expect(delta.updated).toHaveLength(0);
    expect(delta.removed).toHaveLength(0);
    expect(delta.isEmpty).toBe(false);
  });

  it('returns empty delta when snapshots are identical', () => {
    const snapshot = makeSnapshot([makeFeatureState('f1')]);

    const delta = computeDiff(snapshot, snapshot);

    expect(delta.isEmpty).toBe(true);
    expect(delta.added).toHaveLength(0);
    expect(delta.updated).toHaveLength(0);
    expect(delta.removed).toHaveLength(0);
  });

  it('detects added features', () => {
    const prev = makeSnapshot([makeFeatureState('f1')]);
    const curr = makeSnapshot([makeFeatureState('f1'), makeFeatureState('f2')]);

    const delta = computeDiff(prev, curr);

    expect(delta.added).toHaveLength(1);
    expect(delta.added[0]?.id).toBe('f2');
    expect(delta.isEmpty).toBe(false);
  });

  it('detects removed features', () => {
    const prev = makeSnapshot([makeFeatureState('f1'), makeFeatureState('f2')]);
    const curr = makeSnapshot([makeFeatureState('f1')]);

    const delta = computeDiff(prev, curr);

    expect(delta.removed).toEqual(['f2']);
    expect(delta.isEmpty).toBe(false);
  });

  it('detects phase change', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { phase: 'development' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { phase: 'testing' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated).toHaveLength(1);
    expect(delta.updated[0]?.id).toBe('f1');
    expect(delta.updated[0]?.changes['phase']).toEqual({ from: 'development', to: 'testing' });
  });

  it('detects assignee change', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { assignee: 'Claude' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { assignee: 'Andy' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated).toHaveLength(1);
    expect(delta.updated[0]?.changes['assignee']).toEqual({ from: 'Claude', to: 'Andy' });
  });

  it('detects assigneeType change', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { assigneeType: 'agent' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { assigneeType: 'human' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['assigneeType']).toEqual({ from: 'agent', to: 'human' });
  });

  it('detects task count changes', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { tasks: { total: 3, completed: 1 } })]);
    const curr = makeSnapshot([makeFeatureState('f1', { tasks: { total: 3, completed: 2 } })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated).toHaveLength(1);
    expect(delta.updated[0]?.changes['tasks']).toEqual({
      from: { total: 3, completed: 1 },
      to: { total: 3, completed: 2 },
    });
  });

  it('detects test count changes', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { tests: { total: 5, passing: 3 } })]);
    const curr = makeSnapshot([makeFeatureState('f1', { tests: { total: 5, passing: 5 } })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['tests']).toEqual({
      from: { total: 5, passing: 3 },
      to: { total: 5, passing: 5 },
    });
  });

  it('detects title change', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { title: 'Old Title' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { title: 'New Title' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['title']).toEqual({ from: 'Old Title', to: 'New Title' });
  });

  it('detects type change (feature to fix)', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { type: 'feature' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { type: 'fix' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['type']).toEqual({ from: 'feature', to: 'fix' });
  });

  it('detects meta change', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { meta: { score: 70 } })]);
    const curr = makeSnapshot([makeFeatureState('f1', { meta: { score: 85 } })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['meta']).toEqual({ from: { score: 70 }, to: { score: 85 } });
  });

  it('detects meta added (undefined to object)', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { meta: undefined })]);
    const curr = makeSnapshot([makeFeatureState('f1', { meta: { score: 82 } })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['meta']).toEqual({ from: undefined, to: { score: 82 } });
  });

  it('detects assignee added (undefined to value)', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { assignee: undefined })]);
    const curr = makeSnapshot([makeFeatureState('f1', { assignee: 'Claude' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['assignee']).toEqual({ from: undefined, to: 'Claude' });
  });

  it('detects assignee removed (value to undefined)', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { assignee: 'Claude' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { assignee: undefined })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['assignee']).toEqual({ from: 'Claude', to: undefined });
  });

  it('detects assigneeType undefined to value', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { assigneeType: undefined })]);
    const curr = makeSnapshot([makeFeatureState('f1', { assigneeType: 'agent' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes['assigneeType']).toEqual({ from: undefined, to: 'agent' });
  });

  it('does not report unchanged fields', () => {
    const prev = makeSnapshot([makeFeatureState('f1', { phase: 'development', assignee: 'Claude' })]);
    const curr = makeSnapshot([makeFeatureState('f1', { phase: 'testing', assignee: 'Claude' })]);

    const delta = computeDiff(prev, curr);

    expect(delta.updated[0]?.changes).toHaveProperty('phase');
    expect(delta.updated[0]?.changes).not.toHaveProperty('assignee');
  });

  it('handles mixed add, update, and remove', () => {
    const prev = makeSnapshot([
      makeFeatureState('f1', { phase: 'development' }),
      makeFeatureState('f2'),
    ]);
    const curr = makeSnapshot([
      makeFeatureState('f1', { phase: 'testing' }),
      makeFeatureState('f3'),
    ]);

    const delta = computeDiff(prev, curr);

    expect(delta.added).toHaveLength(1);
    expect(delta.added[0]?.id).toBe('f3');
    expect(delta.updated).toHaveLength(1);
    expect(delta.updated[0]?.id).toBe('f1');
    expect(delta.removed).toEqual(['f2']);
    expect(delta.isEmpty).toBe(false);
  });

  it('returns empty for first push with no features', () => {
    const delta = computeDiff(undefined, makeSnapshot([]));

    expect(delta.isEmpty).toBe(true);
    expect(delta.added).toHaveLength(0);
  });

  it('handles features across multiple epics', () => {
    const prev: StateSnapshot = {
      version: 1,
      epics: [
        { id: 'e1', title: 'E1', status: 'in_progress', features: [makeFeatureState('f1', { phase: 'development' })] },
        { id: 'e2', title: 'E2', status: 'in_progress', features: [makeFeatureState('f2')] },
      ],
      computedAt: '',
    };
    const curr: StateSnapshot = {
      version: 1,
      epics: [
        { id: 'e1', title: 'E1', status: 'in_progress', features: [makeFeatureState('f1', { phase: 'done' })] },
        { id: 'e2', title: 'E2', status: 'in_progress', features: [makeFeatureState('f2')] },
      ],
      computedAt: '',
    };

    const delta = computeDiff(prev, curr);

    expect(delta.updated).toHaveLength(1);
    expect(delta.updated[0]?.id).toBe('f1');
  });
});
