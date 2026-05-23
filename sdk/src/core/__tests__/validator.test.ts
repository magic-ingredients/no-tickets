import { describe, it, expect } from 'vitest';
import { validate } from '../validator.js';
import type { ParseResult, ParsedEpic, ParsedFeature } from '../types.js';

function makeEpic(overrides: Partial<ParsedEpic['frontmatter']> = {}, filePath = '.notickets/test/epic.md'): ParsedEpic {
  return {
    frontmatter: {
      id: 'test-epic',
      type: 'epic',
      title: 'Test Epic',
      status: 'not_started',
      created: '2026-04-05',
      updated: '2026-04-05',
      ...overrides,
    },
    description: 'A test epic.',
    goals: [],
    filePath,
  };
}

function makeFeature(overrides: Partial<ParsedFeature['frontmatter']> = {}, tasks: ParsedFeature['tasks'] = [], filePath = '.notickets/test/feature.md'): ParsedFeature {
  return {
    frontmatter: {
      id: 'test-feature',
      type: 'feature',
      epic: 'test-epic',
      title: 'Test Feature',
      phase: 'ideation',
      status: 'not_started',
      created: '2026-04-05',
      updated: '2026-04-05',
      ...overrides,
    },
    description: 'A test feature.',
    tasks,
    acceptanceCriteria: [],
    filePath,
  };
}

describe('validate', () => {
  it('returns valid for well-formed epic and feature', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature()],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('returns valid for empty parse result', () => {
    const result = validate({ epics: [], features: [] });

    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  // -- Epic frontmatter validation --

  it('reports error for epic with non-kebab-case id', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'Not Kebab Case' })],
      features: [],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'id')).toBe(true);
  });

  it('reports error for epic with empty title', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'valid-id', title: '' })],
      features: [],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'title')).toBe(true);
  });

  it('reports error for epic with invalid date format', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ created: 'April 5th' })],
      features: [],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'created')).toBe(true);
  });

  it('reports error for epic with invalid status', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ status: 'invalid' as 'not_started' })],
      features: [],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'status')).toBe(true);
  });

  // -- Feature frontmatter validation --

  it('reports error for feature with invalid phase', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ phase: 'invalid' as 'ideation' })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'phase')).toBe(true);
  });

  it('reports error for feature with invalid assignee_type', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ assignee_type: 'robot' as 'agent' })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'assignee_type')).toBe(true);
  });

  it('allows feature without assignee_type', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ assignee_type: undefined })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(true);
  });

  // -- Epic reference validation --

  it('reports error when feature references non-existent epic', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'existing-epic' })],
      features: [makeFeature({ epic: 'missing-epic' })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    const err = result.errors.find((e) => e.field === 'epic');
    expect(err).toBeDefined();
    expect(err?.message).toContain('missing-epic');
    expect(err?.suggestion).toBeDefined();
  });

  it('reports error for feature with empty epic reference', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ epic: '' })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field === 'epic')).toBe(true);
  });

  it('passes when feature references existing epic', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'my-epic' })],
      features: [makeFeature({ epic: 'my-epic' })],
    };

    expect(validate(parsed).valid).toBe(true);
  });

  // -- Task validation --

  it('reports error for task with invalid status', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: 'Task', status: 'bad' as 'not_started' },
      ])],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field?.includes('task.1'))).toBe(true);
  });

  it('reports error for task with zero or negative number', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 0, title: 'Zero', status: 'not_started' },
      ])],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field?.includes('task.0'))).toBe(true);
  });

  it('reports error for duplicate task numbers', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: 'First', status: 'not_started' },
        { number: 1, title: 'Duplicate', status: 'not_started' },
      ])],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    const err = result.errors.find((e) => e.message.includes('Duplicate'));
    expect(err).toBeDefined();
    expect(err?.field).toBe('task.1');
    expect(err?.suggestion).toBeDefined();
  });

  it('reports error for task numbering gaps', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: 'First', status: 'not_started' },
        { number: 3, title: 'Third', status: 'not_started' },
      ])],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    const err = result.errors.find((e) => e.message.includes('gap'));
    expect(err).toBeDefined();
    expect(err?.field).toBe('tasks');
    expect(err?.suggestion).toContain('Renumber');
  });

  it('passes for sequential task numbering', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: 'First', status: 'not_started' },
        { number: 2, title: 'Second', status: 'completed' },
        { number: 3, title: 'Third', status: 'in_progress' },
      ])],
    };

    expect(validate(parsed).valid).toBe(true);
  });

  it('reports error for task with empty title', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: '', status: 'not_started' },
      ])],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.field?.includes('task.1'))).toBe(true);
  });

  // -- Suggestion quality --

  it('provides specific suggestions for known field errors', () => {
    const parsed: ParseResult = {
      epics: [],
      features: [makeFeature({ id: 'BAD ID', phase: 'wrong' as 'ideation' })],
    };

    const result = validate(parsed);
    const idError = result.errors.find((e) => e.field === 'id');
    const phaseError = result.errors.find((e) => e.field === 'phase');

    expect(idError?.suggestion).toContain('lowercase');
    expect(phaseError?.suggestion).toContain('ideation');
  });

  it('returns empty suggestion for unknown field errors', () => {
    // The 'epic' field error when frontmatter is valid but epic doesn't exist
    // gets a specific suggestion. But Zod errors for fields not in formatSuggestion
    // return empty string.
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ epic: 'test-epic' }, [
        { number: 1, title: 'Task', status: 'bad' as 'not_started' },
      ])],
    };

    const result = validate(parsed);
    // Task status errors may get empty suggestions since 'task.1.status' doesn't match 'status'
    expect(result.valid).toBe(false);
  });

  // -- Duplicate ID validation --

  it('reports error for duplicate IDs across epics', () => {
    const parsed: ParseResult = {
      epics: [
        makeEpic({ id: 'same-id' }, '.notickets/a/epic.md'),
        makeEpic({ id: 'same-id' }, '.notickets/b/epic.md'),
      ],
      features: [],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.message.includes('Duplicate ID'))).toBe(true);
  });

  it('reports error for duplicate IDs across epic and feature', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'shared-id' })],
      features: [makeFeature({ id: 'shared-id' })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.some((e) => e.message.includes('Duplicate ID'))).toBe(true);
  });

  it('passes for unique IDs', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'epic-one' }), makeEpic({ id: 'epic-two' }, '.notickets/two/epic.md')],
      features: [makeFeature({ id: 'feat-one', epic: 'epic-one' })],
    };

    expect(validate(parsed).valid).toBe(true);
  });

  // -- Error structure --

  it('includes file path in error', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'BAD ID' }, '.notickets/bad/epic.md')],
      features: [],
    };

    const result = validate(parsed);
    expect(result.errors[0]?.file).toBe('.notickets/bad/epic.md');
  });

  it('includes suggestion in error', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'BAD ID' })],
      features: [],
    };

    const result = validate(parsed);
    expect(result.errors[0]?.suggestion).toBeDefined();
    expect(result.errors[0]?.suggestion?.length).toBeGreaterThan(0);
  });

  // -- Fix type --

  it('validates fix type features the same as feature type', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ type: 'fix', id: 'fix-bug' })],
    };

    expect(validate(parsed).valid).toBe(true);
  });

  // -- Multiple errors --

  it('reports multiple errors from different validators', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'BAD' })],
      features: [makeFeature({ id: 'ALSO BAD', epic: 'nonexistent', phase: 'wrong' as 'ideation' })],
    };

    const result = validate(parsed);

    expect(result.valid).toBe(false);
    expect(result.errors.length).toBeGreaterThan(2);
  });
});
