import { describe, it, expect } from 'vitest';
import {
  parseFrontmatter,
  parseTasks,
  parseFiles,
  extractSection,
  parseGoals,
  parseAcceptanceCriteria,
  parseDescription,
  assembleEpic,
  assembleFeature,
} from '../parser.js';

/**
 * Tests targeting surviving Stryker mutants.
 * Each test kills one or more specific mutations.
 */

// -- parseFrontmatter mutants (date coercion, empty strings) ------------------

describe('parseFrontmatter — mutant killers', () => {
  it('preserves created/updated as strings, not empty', () => {
    const { data } = parseFrontmatter(`---
created: 2026-04-05
updated: 2026-04-06
---
Body.`);
    expect(data['created']).toBe('2026-04-05');
    expect(data['updated']).toBe('2026-04-06');
    expect(data['created']).not.toBe('');
    expect(data['updated']).not.toBe('');
  });
});

// -- parseTasks mutants (regex anchors, status parsing, loop bounds) ----------

describe('parseTasks — mutant killers', () => {
  it('requires ### heading prefix (not just digits)', () => {
    const body = `## Tasks

1. Not a task heading
status: completed

### 1. Real task
status: completed`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.title).toBe('Real task');
  });

  it('requires $ anchor — does not match mid-line', () => {
    const body = `## Tasks

### 1. Task with trailing text
status: in_progress`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.title).toBe('Task with trailing text');
  });

  it('handles multi-digit task numbers', () => {
    const body = `## Tasks

### 12. Twelfth task
status: completed`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.number).toBe(12);
  });

  it('requires space after ### (not ###1)', () => {
    const body = `## Tasks

###1. No space after hash
status: completed

### 1. With space
status: completed`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.title).toBe('With space');
  });

  it('requires space after digit dot', () => {
    const body = `## Tasks

### 1.No space after dot
status: completed

### 1. With space
status: completed`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(1);
  });

  it('status regex requires start-of-line anchor', () => {
    const body = `## Tasks

### 1. Task
status: completed

The previous status: not_started was wrong.`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.status).toBe('completed');
  });

  it('status regex handles optional whitespace after colon', () => {
    const body = `## Tasks

### 1. No space
status:completed

### 2. With space
status: in_progress`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(2);
    expect(tasks[0]?.status).toBe('completed');
    expect(tasks[1]?.status).toBe('in_progress');
  });

  it('skips empty lines between heading and status', () => {
    const body = `## Tasks

### 1. Task with gap

status: completed`;

    const tasks = parseTasks(body);
    // Status line follows an empty line — parser looks for next non-empty
    expect(tasks).toHaveLength(1);
  });

  it('defaults to not_started, not empty string', () => {
    const body = `## Tasks

### 1. No status line

Just a description.`;

    const tasks = parseTasks(body);
    expect(tasks[0]?.status).toBe('not_started');
    expect(tasks[0]?.status).not.toBe('');
  });
});

// -- extractSection mutants ---------------------------------------------------

describe('extractSection — mutant killers', () => {
  it('matches exact section name, not substring', () => {
    const body = `## Goals Extended

Wrong section.

## Goals

Right section.`;

    expect(extractSection(body, 'Goals')).toBe('Right section.');
  });
});

// -- parseGoals mutants (regex, filter) ---------------------------------------

describe('parseGoals — mutant killers', () => {
  it('strips bullet markers including *', () => {
    const body = `## Goals

- Dash goal
* Star goal`;

    const goals = parseGoals(body);
    expect(goals[0]).toBe('Dash goal');
    expect(goals[1]).toBe('Star goal');
    expect(goals[0]).not.toMatch(/^[-*]/);
  });

  it('filters out empty lines', () => {
    const body = `## Goals

- First

- Second`;

    const goals = parseGoals(body);
    expect(goals).toHaveLength(2);
  });

  it('strips leading whitespace from bullets', () => {
    const body = `## Goals

-  Extra space goal`;

    expect(parseGoals(body)[0]).toBe('Extra space goal');
  });
});

// -- parseAcceptanceCriteria mutants (regex, filter) --------------------------

describe('parseAcceptanceCriteria — mutant killers', () => {
  it('strips checkbox syntax including dash prefix', () => {
    const body = `## Acceptance Criteria

- [ ] Unchecked
- [x] Checked`;

    const criteria = parseAcceptanceCriteria(body);
    expect(criteria[0]).toBe('Unchecked');
    expect(criteria[1]).toBe('Checked');
    expect(criteria[0]).not.toMatch(/^-/);
    expect(criteria[0]).not.toMatch(/\[/);
  });

  it('handles dash-only bullets (no checkbox)', () => {
    const body = `## Acceptance Criteria

- Plain item`;

    expect(parseAcceptanceCriteria(body)[0]).toBe('Plain item');
  });

  it('filters out empty lines between criteria', () => {
    const body = `## Acceptance Criteria

- [ ] First

- [ ] Second`;

    expect(parseAcceptanceCriteria(body)).toHaveLength(2);
  });
});

// -- parseDescription mutants (regex) -----------------------------------------

describe('parseDescription — mutant killers', () => {
  it('strips # heading line from description', () => {
    const body = `# My Feature

Description text.`;

    const desc = parseDescription(body);
    expect(desc).toBe('Description text.');
    expect(desc).not.toContain('# My Feature');
  });

  it('requires # at start of line for heading strip', () => {
    const body = `Not a # heading

Description.`;

    const desc = parseDescription(body);
    expect(desc).toContain('Not a # heading');
  });
});

// -- parseMeta mutants (typeof check, null check) ----------------------------

describe('assembleEpic — meta mutant killers', () => {
  it('returns undefined meta when meta is a string', () => {
    const data = { id: 'test', type: 'epic', title: 'Test', status: 'not_started', created: '2026-01-01', updated: '2026-01-01', meta: 'not an object' };
    const epic = assembleEpic(data, '', 'path.md');
    expect(epic.frontmatter.meta).toBeUndefined();
  });

  it('returns undefined meta when meta is null', () => {
    const data = { id: 'test', type: 'epic', title: 'Test', status: 'not_started', created: '2026-01-01', updated: '2026-01-01', meta: null };
    const epic = assembleEpic(data, '', 'path.md');
    expect(epic.frontmatter.meta).toBeUndefined();
  });

  it('returns meta object when meta is a valid object', () => {
    const data = { id: 'test', type: 'epic', title: 'Test', status: 'not_started', created: '2026-01-01', updated: '2026-01-01', meta: { score: 82 } };
    const epic = assembleEpic(data, '', 'path.md');
    expect(epic.frontmatter.meta).toEqual({ score: 82 });
  });

  it('returns undefined meta when meta is missing', () => {
    const data = { id: 'test', type: 'epic', title: 'Test', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    const epic = assembleEpic(data, '', 'path.md');
    expect(epic.frontmatter.meta).toBeUndefined();
  });
});

// -- normalizeStatus mutants --------------------------------------------------

describe('assembleEpic — normalizeStatus mutant killers', () => {
  it('normalizes in_progress status', () => {
    const data = { id: 'x', type: 'epic', title: 'X', status: 'in_progress', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleEpic(data, '', 'p.md').frontmatter.status).toBe('in_progress');
  });

  it('normalizes completed status', () => {
    const data = { id: 'x', type: 'epic', title: 'X', status: 'completed', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleEpic(data, '', 'p.md').frontmatter.status).toBe('completed');
  });

  it('defaults invalid status to not_started', () => {
    const data = { id: 'x', type: 'epic', title: 'X', status: 'invalid', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleEpic(data, '', 'p.md').frontmatter.status).toBe('not_started');
  });

  it('defaults missing status to not_started', () => {
    const data = { id: 'x', type: 'epic', title: 'X', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleEpic(data, '', 'p.md').frontmatter.status).toBe('not_started');
  });
});

// -- normalizePhase mutants ---------------------------------------------------

describe('assembleFeature — normalizePhase mutant killers', () => {
  it('normalizes each valid phase', () => {
    const base = { id: 'x', type: 'feature', epic: 'e', title: 'X', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };

    expect(assembleFeature({ ...base, phase: 'ideation' }, '', 'p.md').frontmatter.phase).toBe('ideation');
    expect(assembleFeature({ ...base, phase: 'development' }, '', 'p.md').frontmatter.phase).toBe('development');
    expect(assembleFeature({ ...base, phase: 'testing' }, '', 'p.md').frontmatter.phase).toBe('testing');
    expect(assembleFeature({ ...base, phase: 'review' }, '', 'p.md').frontmatter.phase).toBe('review');
    expect(assembleFeature({ ...base, phase: 'done' }, '', 'p.md').frontmatter.phase).toBe('done');
  });

  it('defaults invalid phase to ideation', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', phase: 'invalid', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.phase).toBe('ideation');
  });

  it('defaults missing phase to ideation', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.phase).toBe('ideation');
  });
});

// -- normalizeAssigneeType mutants --------------------------------------------

describe('assembleFeature — normalizeAssigneeType mutant killers', () => {
  it('normalizes human assignee type', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', assignee_type: 'human', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.assignee_type).toBe('human');
  });

  it('normalizes agent assignee type', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', assignee_type: 'agent', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.assignee_type).toBe('agent');
  });

  it('returns undefined for invalid assignee type', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', assignee_type: 'robot', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.assignee_type).toBeUndefined();
  });

  it('returns undefined for missing assignee type', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.assignee_type).toBeUndefined();
  });
});

// -- assembleFeature field mutants (String coercion, empty defaults) ----------

describe('assembleFeature — field coercion mutant killers', () => {
  it('coerces all string fields from data', () => {
    const data = {
      id: 'my-feat',
      type: 'feature',
      epic: 'my-epic',
      title: 'My Feature',
      phase: 'development',
      status: 'in_progress',
      assignee: 'Claude',
      created: '2026-04-05',
      updated: '2026-04-06',
    };
    const f = assembleFeature(data, '', 'p.md');
    expect(f.frontmatter.id).toBe('my-feat');
    expect(f.frontmatter.epic).toBe('my-epic');
    expect(f.frontmatter.title).toBe('My Feature');
    expect(f.frontmatter.assignee).toBe('Claude');
    expect(f.frontmatter.created).toBe('2026-04-05');
    expect(f.frontmatter.updated).toBe('2026-04-06');
  });

  it('defaults missing fields to empty string, not Stryker sentinel', () => {
    const data = { type: 'feature' };
    const f = assembleFeature(data, '', 'p.md');
    expect(f.frontmatter.id).toBe('');
    expect(f.frontmatter.epic).toBe('');
    expect(f.frontmatter.title).toBe('');
    expect(f.frontmatter.created).toBe('');
    expect(f.frontmatter.updated).toBe('');
  });

  it('returns undefined assignee when not provided', () => {
    const data = { id: 'x', type: 'feature', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.assignee).toBeUndefined();
  });

  it('handles fix type correctly', () => {
    const data = { id: 'x', type: 'fix', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.type).toBe('fix');
  });

  it('defaults unknown type to feature', () => {
    const data = { id: 'x', type: 'unknown', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' };
    expect(assembleFeature(data, '', 'p.md').frontmatter.type).toBe('feature');
  });
});

// -- parseTasks loop/trim mutants ---------------------------------------------

describe('parseTasks — loop and trim mutants', () => {
  it('trims whitespace from task titles', () => {
    const body = `## Tasks

### 1.  Padded title
status: completed`;

    const tasks = parseTasks(body);
    expect(tasks[0]?.title).toBe('Padded title');
  });

  it('trims whitespace from status line', () => {
    const body = `## Tasks

### 1. Task
  status: completed  `;

    const tasks = parseTasks(body);
    expect(tasks[0]?.status).toBe('completed');
  });

  it('parses all tasks in sequence, not just first', () => {
    const body = `## Tasks

### 1. First
status: completed

### 2. Second
status: in_progress

### 3. Third
status: not_started`;

    const tasks = parseTasks(body);
    expect(tasks).toHaveLength(3);
    expect(tasks[2]?.number).toBe(3);
    expect(tasks[2]?.status).toBe('not_started');
  });

  it('handles task at end of file with no trailing newline', () => {
    const body = `## Tasks

### 1. Last task
status: completed`;

    expect(parseTasks(body)).toHaveLength(1);
    expect(parseTasks(body)[0]?.status).toBe('completed');
  });

  it('skips empty lines in task section', () => {
    const body = `## Tasks


### 1. After empty lines
status: completed`;

    expect(parseTasks(body)).toHaveLength(1);
  });
});

// -- parseGoals regex precision mutants ---------------------------------------

describe('parseGoals — regex precision mutants', () => {
  it('preserves text after bullet marker with no extra stripping', () => {
    const body = `## Goals

- Goal with -dashes- in text`;

    expect(parseGoals(body)[0]).toBe('Goal with -dashes- in text');
  });

  it('handles bullet with multiple spaces after marker', () => {
    const body = `## Goals

-   Triple spaced`;

    expect(parseGoals(body)[0]).toBe('Triple spaced');
  });
});

// -- parseAcceptanceCriteria regex precision mutants ---------------------------

describe('parseAcceptanceCriteria — regex precision mutants', () => {
  it('preserves text with brackets in content', () => {
    const body = `## Acceptance Criteria

- [ ] Must handle [special] characters`;

    expect(parseAcceptanceCriteria(body)[0]).toBe('Must handle [special] characters');
  });

  it('strips both checked and unchecked checkboxes identically', () => {
    const body = `## Acceptance Criteria

- [ ] Unchecked item
- [x] Checked item`;

    const criteria = parseAcceptanceCriteria(body);
    expect(criteria[0]).not.toMatch(/^\[/);
    expect(criteria[1]).not.toMatch(/^\[/);
  });

  it('handles no-space after checkbox', () => {
    const body = `## Acceptance Criteria

- [ ]No space after checkbox`;

    const criteria = parseAcceptanceCriteria(body);
    expect(criteria[0]).toBe('No space after checkbox');
  });
});

// -- parseDescription regex mutants -------------------------------------------

describe('parseDescription — regex precision mutants', () => {
  it('only strips first # heading line, preserves rest', () => {
    const body = `# Title

Line 1
# Not stripped (not at start)`;

    const desc = parseDescription(body);
    expect(desc).toContain('Line 1');
    // The second # line should be preserved since only the first is stripped
  });
});

// -- parseFiles string literal mutants ----------------------------------------

describe('parseFiles — string literal mutants', () => {
  it('recognizes fix type documents', () => {
    const result = assembleFeature({ id: 'fix-bug', type: 'fix', epic: 'e', title: 'Fix Bug', phase: 'testing', status: 'in_progress', created: '2026-01-01', updated: '2026-01-01' }, '', 'fix.md');
    expect(result.frontmatter.type).toBe('fix');
  });

  it('ignores documents with unrecognized type via parseFiles', () => {
    const files = [{
      path: 'other.md',
      content: `---
id: other
type: unknown
title: Other
---

# Other`,
    }];

    const result = parseFiles(files);
    expect(result.epics).toHaveLength(0);
    expect(result.features).toHaveLength(0);
  });
});

// -- normalizer default string mutants ----------------------------------------

describe('normalizers — default value mutants', () => {
  it('normalizeStatus defaults to not_started string, not empty', () => {
    const e = assembleEpic({ id: 'x', type: 'epic', title: 'X', created: '2026-01-01', updated: '2026-01-01' }, '', 'p.md');
    expect(e.frontmatter.status).toBe('not_started');
    expect(e.frontmatter.status.length).toBeGreaterThan(0);
  });

  it('normalizePhase defaults to ideation string, not empty', () => {
    const f = assembleFeature({ id: 'x', type: 'feature', epic: 'e', title: 'X', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' }, '', 'p.md');
    expect(f.frontmatter.phase).toBe('ideation');
    expect(f.frontmatter.phase.length).toBeGreaterThan(0);
  });

  it('assembleFeature default type fallback is feature string', () => {
    const f = assembleFeature({ id: 'x', epic: 'e', title: 'X', phase: 'ideation', status: 'not_started', created: '2026-01-01', updated: '2026-01-01' }, '', 'p.md');
    expect(f.frontmatter.type).toBe('feature');
    expect(f.frontmatter.type.length).toBeGreaterThan(0);
  });
});

// -- assembleEpic field mutants -----------------------------------------------

describe('assembleEpic — field coercion mutant killers', () => {
  it('coerces all string fields from data', () => {
    const data = { id: 'my-epic', type: 'epic', title: 'My Epic', status: 'completed', created: '2026-04-05', updated: '2026-04-06' };
    const e = assembleEpic(data, '', 'p.md');
    expect(e.frontmatter.id).toBe('my-epic');
    expect(e.frontmatter.title).toBe('My Epic');
    expect(e.frontmatter.created).toBe('2026-04-05');
    expect(e.frontmatter.updated).toBe('2026-04-06');
  });

  it('defaults missing fields to empty string', () => {
    const data = { type: 'epic' };
    const e = assembleEpic(data, '', 'p.md');
    expect(e.frontmatter.id).toBe('');
    expect(e.frontmatter.title).toBe('');
    expect(e.frontmatter.created).toBe('');
    expect(e.frontmatter.updated).toBe('');
  });
});
