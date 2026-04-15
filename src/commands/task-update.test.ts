import { describe, it, expect } from 'vitest';
import { parseTaskUpdateArgs } from './task-update.js';

describe('parseTaskUpdateArgs', () => {
  it('parses minimal args: --task and --status', () => {
    const result = parseTaskUpdateArgs(['--task', 'task-1', '--status', 'in_progress']);

    expect(result.success).toBe(true);
    if (!result.success) return;
    expect(result.data.taskId).toBe('task-1');
    expect(result.data.status).toBe('in_progress');
    expect(result.data.phase).toBeUndefined();
    expect(result.data.commitSha).toBeUndefined();
  });

  it('parses all optional flags', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--phase', 'complete',
      '--commit-sha', 'abc123',
      '--coverage', '94',
      '--tests-total', '12',
      '--tests-passing', '12',
      '--review-verdict', 'approved',
      '--documentation',
    ]);

    expect(result.success).toBe(true);
    if (!result.success) return;
    expect(result.data.taskId).toBe('task-1');
    expect(result.data.status).toBe('completed');
    expect(result.data.phase).toBe('complete');
    expect(result.data.commitSha).toBe('abc123');
    expect(result.data.coverage).toBe(94);
    expect(result.data.testsTotal).toBe(12);
    expect(result.data.testsPassing).toBe(12);
    expect(result.data.reviewVerdict).toBe('approved');
    expect(result.data.documentation).toBe(true);
  });

  it('returns error when --task is missing', () => {
    const result = parseTaskUpdateArgs(['--status', 'in_progress']);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('--task');
  });

  it('returns error when --status is missing', () => {
    const result = parseTaskUpdateArgs(['--task', 'task-1']);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('--status');
  });

  it('returns error for invalid status value', () => {
    const result = parseTaskUpdateArgs(['--task', 'task-1', '--status', 'invalid']);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('status');
  });

  it('returns error for invalid phase value', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'in_progress',
      '--phase', 'invalid',
    ]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('phase');
  });

  it('returns error for non-numeric coverage', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--coverage', 'abc',
    ]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('coverage');
  });

  it('returns error for coverage out of range', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--coverage', '101',
    ]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('coverage');
  });

  it('handles --documentation as boolean flag (no value)', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--documentation',
    ]);

    expect(result.success).toBe(true);
    if (!result.success) return;
    expect(result.data.documentation).toBe(true);
  });

  it('returns error for empty args', () => {
    const result = parseTaskUpdateArgs([]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('--task');
  });

  it('returns error when flag value is missing (next arg is a flag)', () => {
    const result = parseTaskUpdateArgs(['--task', '--status', 'in_progress']);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('--task');
  });

  it('returns error for negative coverage', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--coverage', '-5',
    ]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('coverage');
  });

  it('returns error for non-integer tests-total', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--tests-total', '3.5',
    ]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('tests-total');
  });

  it('returns error for non-integer tests-passing', () => {
    const result = parseTaskUpdateArgs([
      '--task', 'task-1',
      '--status', 'completed',
      '--tests-passing', '2.7',
    ]);

    expect(result.success).toBe(false);
    if (result.success) return;
    expect(result.error).toContain('tests-passing');
  });

  it('accepts coverage boundary values: 0 and 100', () => {
    const zero = parseTaskUpdateArgs(['--task', 't', '--status', 'completed', '--coverage', '0']);
    expect(zero.success).toBe(true);
    if (zero.success) expect(zero.data.coverage).toBe(0);

    const hundred = parseTaskUpdateArgs(['--task', 't', '--status', 'completed', '--coverage', '100']);
    expect(hundred.success).toBe(true);
    if (hundred.success) expect(hundred.data.coverage).toBe(100);
  });

  it('rejects coverage at -1 and 101', () => {
    const neg = parseTaskUpdateArgs(['--task', 't', '--status', 'completed', '--coverage', '-1']);
    expect(neg.success).toBe(false);

    const over = parseTaskUpdateArgs(['--task', 't', '--status', 'completed', '--coverage', '101']);
    expect(over.success).toBe(false);
  });

  it('accepts zero for tests-total and tests-passing', () => {
    const result = parseTaskUpdateArgs([
      '--task', 't', '--status', 'completed',
      '--tests-total', '0', '--tests-passing', '0',
    ]);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.testsTotal).toBe(0);
      expect(result.data.testsPassing).toBe(0);
    }
  });

  it('rejects non-numeric tests-total', () => {
    const result = parseTaskUpdateArgs(['--task', 't', '--status', 'completed', '--tests-total', 'abc']);
    expect(result.success).toBe(false);
    if (!result.success) expect(result.error).toContain('tests-total');
  });

  it('returns only taskId and status when no optional flags provided', () => {
    const result = parseTaskUpdateArgs(['--task', 'task-1', '--status', 'in_progress']);
    expect(result.success).toBe(true);
    if (!result.success) return;
    expect(Object.keys(result.data)).toEqual(['taskId', 'status']);
  });
});
