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

describe('parseFrontmatter', () => {
  it('extracts YAML frontmatter and body', () => {
    const content = `---
id: my-epic
type: epic
title: My Epic
status: not_started
created: 2026-04-05
updated: 2026-04-05
---

# My Epic

Some description here.`;

    const { data, body } = parseFrontmatter(content);

    expect(data).toEqual({
      id: 'my-epic',
      type: 'epic',
      title: 'My Epic',
      status: 'not_started',
      created: '2026-04-05',
      updated: '2026-04-05',
    });
    expect(body.trim()).toBe('# My Epic\n\nSome description here.');
  });

  it('returns empty data for content without frontmatter', () => {
    const { data, body } = parseFrontmatter('# Just a heading\n\nSome text.');

    expect(data).toEqual({});
    expect(body).toBe('# Just a heading\n\nSome text.');
  });

  it('handles values containing colons', () => {
    const content = `---
title: "My Feature: The Sequel"
status: not_started
---

Body.`;

    const { data } = parseFrontmatter(content);

    expect(data['title']).toBe('My Feature: The Sequel');
  });

  it('handles empty frontmatter', () => {
    const content = `---
---

Body text.`;

    const { data, body } = parseFrontmatter(content);

    expect(data).toEqual({});
    expect(body.trim()).toBe('Body text.');
  });
});

describe('parseTasks', () => {
  it('extracts numbered tasks with status', () => {
    const body = `## Description

Some description.

## Tasks

### 1. Create verification endpoint
status: completed

Build the endpoint.

### 2. Build email template
status: in_progress

Design the template.

### 3. Add status tracking
status: not_started

Track verification status.`;

    const tasks = parseTasks(body);

    expect(tasks).toHaveLength(3);
    expect(tasks[0]).toEqual({ number: 1, title: 'Create verification endpoint', status: 'completed' });
    expect(tasks[1]).toEqual({ number: 2, title: 'Build email template', status: 'in_progress' });
    expect(tasks[2]).toEqual({ number: 3, title: 'Add status tracking', status: 'not_started' });
  });

  it('defaults to not_started when status line is missing', () => {
    const body = `## Tasks

### 1. A task without status

Just a description.`;

    const tasks = parseTasks(body);

    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.status).toBe('not_started');
  });

  it('returns empty array when no Tasks section', () => {
    const body = `## Description

No tasks here.`;

    expect(parseTasks(body)).toEqual([]);
  });

  it('ignores invalid status values', () => {
    const body = `## Tasks

### 1. Bad status task
status: invalid_value

Description.`;

    const tasks = parseTasks(body);

    expect(tasks).toHaveLength(1);
    expect(tasks[0]?.status).toBe('not_started');
  });
});

describe('extractSection', () => {
  it('extracts content between ## headings', () => {
    const body = `## First

Content one.

## Second

Content two.

## Third

Content three.`;

    expect(extractSection(body, 'Second')).toBe('Content two.');
  });

  it('extracts last section until end of document', () => {
    const body = `## First

Content one.

## Last

Content last.`;

    expect(extractSection(body, 'Last')).toBe('Content last.');
  });

  it('returns undefined for missing section', () => {
    expect(extractSection('## Other\n\nContent.', 'Missing')).toBeUndefined();
  });

  it('handles section names with regex metacharacters', () => {
    const body = `## Tasks (v2)

Content here.

## Other

More content.`;

    expect(extractSection(body, 'Tasks (v2)')).toBe('Content here.');
  });
});

describe('parseGoals', () => {
  it('extracts bulleted goals', () => {
    const body = `## Goals

- Reduce drop-off rate
- Verify email addresses
- Collect profile data`;

    const goals = parseGoals(body);

    expect(goals).toEqual(['Reduce drop-off rate', 'Verify email addresses', 'Collect profile data']);
  });

  it('handles asterisk bullets', () => {
    const body = `## Goals

* Goal one
* Goal two`;

    expect(parseGoals(body)).toEqual(['Goal one', 'Goal two']);
  });

  it('returns empty array when no Goals section', () => {
    expect(parseGoals('## Other\n\nContent.')).toEqual([]);
  });
});

describe('parseAcceptanceCriteria', () => {
  it('extracts checkbox items', () => {
    const body = `## Acceptance Criteria

- [ ] Users can verify email
- [x] API endpoint returns 200
- [ ] Email template renders`;

    const criteria = parseAcceptanceCriteria(body);

    expect(criteria).toEqual([
      'Users can verify email',
      'API endpoint returns 200',
      'Email template renders',
    ]);
  });

  it('returns empty array when no Acceptance Criteria section', () => {
    expect(parseAcceptanceCriteria('## Other\n\nContent.')).toEqual([]);
  });

  it('handles plain bullet items without checkboxes', () => {
    const body = `## Acceptance Criteria

- First criterion
- Second criterion`;

    expect(parseAcceptanceCriteria(body)).toEqual(['First criterion', 'Second criterion']);
  });

  it('handles empty section', () => {
    const body = `## Acceptance Criteria

## Next Section`;

    expect(parseAcceptanceCriteria(body)).toEqual([]);
  });
});

describe('parseDescription', () => {
  it('extracts text before first ## heading', () => {
    const body = `# Feature Title

This is the description of the feature.
It spans multiple lines.

## Tasks

### 1. First task
status: not_started`;

    expect(parseDescription(body)).toBe(
      'This is the description of the feature.\nIt spans multiple lines.',
    );
  });

  it('returns full body when no ## headings exist', () => {
    const body = `# Title

Just a description.`;

    expect(parseDescription(body)).toBe('Just a description.');
  });
});

describe('assembleEpic', () => {
  it('builds a ParsedEpic from data and body', () => {
    const data = {
      id: 'onboarding',
      type: 'epic',
      title: 'User Onboarding',
      status: 'in_progress',
      created: '2026-04-05',
      updated: '2026-04-05',
    };
    const body = `# User Onboarding

Build the onboarding flow.

## Goals

- Fast signup
- Email verification`;

    const epic = assembleEpic(data, body, '.notickets/onboarding/epic.md');

    expect(epic.frontmatter.id).toBe('onboarding');
    expect(epic.frontmatter.type).toBe('epic');
    expect(epic.frontmatter.title).toBe('User Onboarding');
    expect(epic.frontmatter.status).toBe('in_progress');
    expect(epic.description).toBe('Build the onboarding flow.');
    expect(epic.goals).toEqual(['Fast signup', 'Email verification']);
    expect(epic.filePath).toBe('.notickets/onboarding/epic.md');
  });

  it('falls back to defaults for missing frontmatter fields', () => {
    const data = {};
    const epic = assembleEpic(data, '# Untitled', 'path.md');

    expect(epic.frontmatter.id).toBe('');
    expect(epic.frontmatter.title).toBe('');
    expect(epic.frontmatter.status).toBe('not_started');
    expect(epic.frontmatter.created).toBe('');
    expect(epic.frontmatter.meta).toBeUndefined();
  });
});

describe('assembleFeature', () => {
  it('builds a ParsedFeature from data and body', () => {
    const data = {
      id: 'email-verify',
      type: 'feature',
      epic: 'onboarding',
      title: 'Email Verification',
      phase: 'development',
      status: 'in_progress',
      assignee: 'Claude',
      assignee_type: 'agent',
      created: '2026-04-05',
      updated: '2026-04-05',
    };
    const body = `# Email Verification

Build the verification flow.

## Acceptance Criteria

- [ ] Sends verification email
- [ ] Validates token

## Tasks

### 1. Create endpoint
status: completed

### 2. Build template
status: not_started`;

    const feature = assembleFeature(data, body, '.notickets/onboarding/email-verify.md');

    expect(feature.frontmatter.id).toBe('email-verify');
    expect(feature.frontmatter.type).toBe('feature');
    expect(feature.frontmatter.epic).toBe('onboarding');
    expect(feature.frontmatter.phase).toBe('development');
    expect(feature.frontmatter.assignee).toBe('Claude');
    expect(feature.frontmatter.assignee_type).toBe('agent');
    expect(feature.tasks).toHaveLength(2);
    expect(feature.tasks[0]).toEqual({ number: 1, title: 'Create endpoint', status: 'completed' });
    expect(feature.tasks[1]).toEqual({ number: 2, title: 'Build template', status: 'not_started' });
    expect(feature.acceptanceCriteria).toEqual(['Sends verification email', 'Validates token']);
  });

  it('handles fix type', () => {
    const data = { id: 'fix-timeout', type: 'fix', epic: 'onboarding', title: 'Fix Timeout', phase: 'testing', status: 'not_started', created: '2026-04-05', updated: '2026-04-05' };
    const feature = assembleFeature(data, '# Fix\n\nDescription.', 'path.md');

    expect(feature.frontmatter.type).toBe('fix');
  });
});

describe('parseFiles', () => {
  it('parses a mix of epics and features', () => {
    const files = [
      {
        path: '.notickets/onboarding/epic.md',
        content: `---
id: onboarding
type: epic
title: User Onboarding
status: not_started
created: 2026-04-05
updated: 2026-04-05
---

# User Onboarding

The onboarding epic.`,
      },
      {
        path: '.notickets/onboarding/email-verify.md',
        content: `---
id: email-verify
type: feature
epic: onboarding
title: Email Verification
phase: ideation
status: not_started
created: 2026-04-05
updated: 2026-04-05
---

# Email Verification

Build email verification.

## Tasks

### 1. Create endpoint
status: not_started`,
      },
    ];

    const result = parseFiles(files);

    expect(result.epics).toHaveLength(1);
    expect(result.features).toHaveLength(1);
    expect(result.epics[0]?.frontmatter.id).toBe('onboarding');
    expect(result.features[0]?.frontmatter.id).toBe('email-verify');
    expect(result.features[0]?.tasks).toHaveLength(1);
  });

  it('skips files without a recognized type', () => {
    const files = [
      { path: 'README.md', content: '# Readme\n\nJust a readme.' },
    ];

    const result = parseFiles(files);

    expect(result.epics).toHaveLength(0);
    expect(result.features).toHaveLength(0);
  });
});
