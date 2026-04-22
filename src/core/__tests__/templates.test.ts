import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { parseFrontmatter, parseTasks, parseGoals, parseAcceptanceCriteria, extractSection } from '../parser.js';

// Real filesystem dependency: reads template files from templates/ directory.
// These are conformance tests verifying templates match the format spec.
const __dirname = dirname(fileURLToPath(import.meta.url));
const TEMPLATES_DIR = join(__dirname, '..', '..', '..', 'templates');

function readTemplate(name: string): string {
  return readFileSync(join(TEMPLATES_DIR, name), 'utf-8');
}

describe('epic template', () => {
  it('has valid YAML frontmatter with required fields', () => {
    const content = readTemplate('epic.md');
    const { data } = parseFrontmatter(content);

    expect(data['type']).toBe('epic');
    expect(typeof data['id']).toBe('string');
    expect((data['id'] as string).length).toBeGreaterThan(0);
    expect(typeof data['title']).toBe('string');
    expect((data['title'] as string).length).toBeGreaterThan(0);
    expect(data['status']).toBe('not_started');
    expect(data['created']).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(data['updated']).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('has a Goals section with items', () => {
    const content = readTemplate('epic.md');
    const { body } = parseFrontmatter(content);

    expect(parseGoals(body).length).toBeGreaterThan(0);
  });

  it('has a Features section', () => {
    const content = readTemplate('epic.md');
    const { body } = parseFrontmatter(content);

    expect(extractSection(body, 'Features')).toBeDefined();
  });
});

describe('feature template', () => {
  it('has valid YAML frontmatter with required fields', () => {
    const content = readTemplate('feature.md');
    const { data } = parseFrontmatter(content);

    expect(data['type']).toBe('feature');
    expect(typeof data['id']).toBe('string');
    expect((data['id'] as string).length).toBeGreaterThan(0);
    expect(typeof data['epic']).toBe('string');
    expect((data['epic'] as string).length).toBeGreaterThan(0);
    expect(typeof data['title']).toBe('string');
    expect(data['phase']).toBe('ideation');
    expect(data['status']).toBe('not_started');
    expect(data['created']).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(data['updated']).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('has a Tasks section with two example tasks all not_started', () => {
    const content = readTemplate('feature.md');
    const { body } = parseFrontmatter(content);
    const tasks = parseTasks(body);

    expect(tasks).toHaveLength(2);
    expect(tasks[0]?.status).toBe('not_started');
    expect(tasks[1]?.status).toBe('not_started');
  });

  it('has an Acceptance Criteria section with items', () => {
    const content = readTemplate('feature.md');
    const { body } = parseFrontmatter(content);

    expect(parseAcceptanceCriteria(body).length).toBeGreaterThan(0);
  });

  it('has a Dependencies section', () => {
    const content = readTemplate('feature.md');
    const { body } = parseFrontmatter(content);

    expect(extractSection(body, 'Dependencies')).toBeDefined();
  });
});

describe('fix template', () => {
  it('has valid YAML frontmatter with required fields including severity', () => {
    const content = readTemplate('fix.md');
    const { data } = parseFrontmatter(content);

    expect(data['type']).toBe('fix');
    expect(typeof data['id']).toBe('string');
    expect((data['id'] as string).length).toBeGreaterThan(0);
    expect(typeof data['epic']).toBe('string');
    expect(data['phase']).toBe('development');
    expect(data['status']).toBe('not_started');
    expect(data['severity']).toBe('medium');
    expect(data['created']).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(data['updated']).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it('has a Tasks section with two example tasks all not_started', () => {
    const content = readTemplate('fix.md');
    const { body } = parseFrontmatter(content);
    const tasks = parseTasks(body);

    expect(tasks).toHaveLength(2);
    expect(tasks[0]?.status).toBe('not_started');
    expect(tasks[1]?.status).toBe('not_started');
  });

  it('has a Reproduction Steps section', () => {
    const content = readTemplate('fix.md');
    const { body } = parseFrontmatter(content);

    expect(extractSection(body, 'Reproduction Steps')).toBeDefined();
  });

  it('has a Root Cause section', () => {
    const content = readTemplate('fix.md');
    const { body } = parseFrontmatter(content);

    expect(extractSection(body, 'Root Cause')).toBeDefined();
  });
});
