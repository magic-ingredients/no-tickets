import { describe, it, expect } from 'vitest';
import { validateFiles } from '../validate.js';
import type { FileEntry } from '../../core/types.js';

function epicFile(id: string): FileEntry {
  return {
    path: `.notickets/${id}/epic.md`,
    content: `---\nid: ${id}\ntype: epic\ntitle: Epic ${id}\nstatus: in_progress\ncreated: 2026-04-22\nupdated: 2026-04-22\n---\n# Epic ${id}\n`,
  };
}

function featureFile(id: string, epicId: string): FileEntry {
  return {
    path: `.notickets/${epicId}/${id}.md`,
    content: `---\nid: ${id}\ntype: feature\nepic: ${epicId}\ntitle: Feature ${id}\nphase: development\nstatus: in_progress\ncreated: 2026-04-22\nupdated: 2026-04-22\n---\n# Feature ${id}\n\n## Tasks\n\n### 1. First task\nstatus: not_started\n`,
  };
}

describe('validateFiles', () => {
  it('returns valid for well-formed files', () => {
    const files = [epicFile('auth'), featureFile('login', 'auth')];
    const result = validateFiles(files);

    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('returns errors for missing required fields', () => {
    const badEpic: FileEntry = {
      path: '.notickets/bad/epic.md',
      content: '---\ntype: epic\n---\n# Bad Epic\n',
    };
    const result = validateFiles([badEpic]);

    expect(result.valid).toBe(false);
    expect(result.errors.length).toBeGreaterThan(0);
  });

  it('returns errors for orphan feature referencing non-existent epic', () => {
    const files = [epicFile('real'), featureFile('orphan', 'ghost')];
    const result = validateFiles(files);

    expect(result.valid).toBe(false);
    const epicError = result.errors.find((e) => e.field === 'epic');
    expect(epicError).toBeDefined();
    expect(epicError?.message).toContain('ghost');
  });

  it('returns valid for empty file list', () => {
    const result = validateFiles([]);

    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('includes file path in validation errors', () => {
    const badFeature: FileEntry = {
      path: '.notickets/auth/broken.md',
      content: '---\nid: broken\ntype: feature\nepic: auth\ntitle: Broken\nphase: invalid-phase\nstatus: in_progress\ncreated: 2026-04-22\nupdated: 2026-04-22\n---\n',
    };
    const result = validateFiles([epicFile('auth'), badFeature]);

    expect(result.valid).toBe(false);
    expect(result.errors[0]?.file).toBe('.notickets/auth/broken.md');
  });
});
