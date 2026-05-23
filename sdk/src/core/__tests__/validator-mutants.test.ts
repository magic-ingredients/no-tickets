import { describe, it, expect } from 'vitest';
import { validate } from '../validator.js';
import type { ParseResult, ParsedEpic, ParsedFeature } from '../types.js';

function makeEpic(overrides: Partial<ParsedEpic['frontmatter']> = {}, filePath = '.notickets/test/epic.md'): ParsedEpic {
  return {
    frontmatter: { id: 'test-epic', type: 'epic', title: 'Test Epic', status: 'not_started', created: '2026-04-05', updated: '2026-04-05', ...overrides },
    description: '', goals: [], filePath,
  };
}

function makeFeature(overrides: Partial<ParsedFeature['frontmatter']> = {}, tasks: ParsedFeature['tasks'] = [], filePath = '.notickets/test/feature.md'): ParsedFeature {
  return {
    frontmatter: { id: 'test-feature', type: 'feature', epic: 'test-epic', title: 'Test Feature', phase: 'ideation', status: 'not_started', created: '2026-04-05', updated: '2026-04-05', ...overrides },
    description: '', tasks, acceptanceCriteria: [], filePath,
  };
}

describe('validate — formatSuggestion mutant killers', () => {
  it('returns type suggestion for invalid type field', () => {
    const parsed: ParseResult = {
      epics: [],
      features: [makeFeature({ type: 'invalid' as 'feature' })],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field === 'type');
    expect(err?.suggestion).toContain('epic');
    expect(err?.suggestion).toContain('feature');
    expect(err?.suggestion).toContain('fix');
  });

  it('returns assignee_type suggestion for invalid assignee_type', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({ assignee_type: 'robot' as 'agent' })],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field === 'assignee_type');
    expect(err?.suggestion).toContain('human');
    expect(err?.suggestion).toContain('agent');
  });

  it('returns date suggestion for invalid date format', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ created: 'not-a-date' })],
      features: [],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field === 'created');
    expect(err?.suggestion).toContain('ISO date');
    expect(err?.suggestion).toContain('2026');
  });

  it('returns empty string suggestion for unknown fields', () => {
    // Force a Zod error on a field not in formatSuggestion
    // The 'title' field with min(1) error won't match any formatSuggestion case
    const parsed: ParseResult = {
      epics: [makeEpic({ title: '' })],
      features: [],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field === 'title');
    expect(err?.suggestion).toBe('');
  });
});

describe('validate — epic reference mutant killers', () => {
  it('includes specific suggestion when epic not found', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'existing' })],
      features: [makeFeature({ epic: 'missing-epic' })],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field === 'epic');
    expect(err?.suggestion).toContain('.notickets/missing-epic/epic.md');
  });

  it('skips epic check when frontmatter is invalid (no false positive)', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'existing' })],
      features: [makeFeature({ id: 'BAD ID', epic: 'nonexistent' })],
    };
    const result = validate(parsed);
    // Should have id error but NOT epic reference error (frontmatter invalid → skip)
    expect(result.errors.some((e) => e.field === 'id')).toBe(true);
    expect(result.errors.some((e) => e.field === 'epic' && e.message.includes('not found'))).toBe(false);
  });

  it('reports epic reference error when frontmatter IS valid but epic missing', () => {
    const parsed: ParseResult = {
      epics: [makeEpic({ id: 'existing' })],
      features: [makeFeature({ epic: 'nonexistent' })],
    };
    const result = validate(parsed);
    expect(result.errors.some((e) => e.field === 'epic' && e.message.includes('not found'))).toBe(true);
  });
});

describe('validate — task validation mutant killers', () => {
  it('includes field path with task number for schema errors', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: '', status: 'not_started' },
      ])],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field?.startsWith('task.1'));
    expect(err).toBeDefined();
    expect(err?.field).toContain('task.1');
  });

  it('includes renumber suggestion for gaps', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 1, title: 'A', status: 'not_started' },
        { number: 3, title: 'C', status: 'not_started' },
      ])],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.message.includes('gap'));
    expect(err?.suggestion).toContain('Renumber');
  });

  it('does not report gap for empty tasks', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [])],
    };
    expect(validate(parsed).valid).toBe(true);
  });

  it('sorts task numbers before gap check', () => {
    // Tasks out of order but sequential — should pass
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [makeFeature({}, [
        { number: 2, title: 'B', status: 'not_started' },
        { number: 1, title: 'A', status: 'not_started' },
        { number: 3, title: 'C', status: 'not_started' },
      ])],
    };
    expect(validate(parsed).valid).toBe(true);
  });
});

describe('validate — duplicate ID mutant killers', () => {
  it('includes conflicting file path in duplicate error message', () => {
    const parsed: ParseResult = {
      epics: [
        makeEpic({ id: 'dupe' }, 'first.md'),
        makeEpic({ id: 'dupe' }, 'second.md'),
      ],
      features: [],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.message.includes('Duplicate'));
    expect(err?.message).toContain('first.md');
    expect(err?.field).toBe('id');
  });

  it('reports duplicate between feature and feature', () => {
    const parsed: ParseResult = {
      epics: [makeEpic()],
      features: [
        makeFeature({ id: 'same' }, [], 'a.md'),
        makeFeature({ id: 'same' }, [], 'b.md'),
      ],
    };
    const result = validate(parsed);
    expect(result.errors.some((e) => e.message.includes('Duplicate ID "same"'))).toBe(true);
  });
});

describe('validate — Zod field path mutant killers', () => {
  it('joins nested Zod paths with dot separator', () => {
    // The path.join('.') call on Zod errors uses '.' as separator
    // Verify by checking that epic frontmatter errors have simple field names
    const parsed: ParseResult = {
      epics: [makeEpic({ status: 'bad' as 'not_started' })],
      features: [],
    };
    const result = validate(parsed);
    const err = result.errors.find((e) => e.field === 'status');
    expect(err).toBeDefined();
    expect(err?.field).not.toContain(','); // join('.') not join(',')
  });
});
