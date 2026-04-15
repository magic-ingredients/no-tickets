import { describe, it, expect } from 'vitest';
import { detectLlmTool, buildInitPlan } from '../commands/init.js';

const TEST_DATE = '2026-04-05';

describe('detectLlmTool', () => {
  it('detects Claude Code from .claude directory', () => {
    expect(detectLlmTool(['.claude', 'src', 'package.json'])).toBe('claude-code');
  });

  it('detects Cursor from .cursor directory', () => {
    expect(detectLlmTool(['.cursor', 'src', 'package.json'])).toBe('cursor');
  });

  it('detects Windsurf from .windsurf directory', () => {
    expect(detectLlmTool(['.windsurf', 'src', 'package.json'])).toBe('windsurf');
  });

  it('returns generic when no tool detected', () => {
    expect(detectLlmTool(['src', 'package.json'])).toBe('generic');
  });

  it('prefers Claude Code over Cursor when both present', () => {
    expect(detectLlmTool(['.claude', '.cursor', 'src'])).toBe('claude-code');
  });
});

describe('buildInitPlan', () => {
  it('creates exact directory list', () => {
    const plan = buildInitPlan('claude-code', TEST_DATE);

    expect(plan.directories).toEqual([
      '.notickets',
      '.notickets/getting-started',
    ]);
  });

  it('produces exactly 4 files', () => {
    expect(buildInitPlan('claude-code', TEST_DATE).files).toHaveLength(4);
  });

  it('includes config.example.json with correct shape', () => {
    const plan = buildInitPlan('claude-code', TEST_DATE);
    const configFile = plan.files.find((f) => f.path === '.notickets/config.example.json');

    expect(configFile).toBeDefined();
    const parsed = JSON.parse(configFile!.content) as Record<string, unknown>;
    expect(parsed).toEqual({
      teamId: 'your-team-id',
      projectId: 'your-project-id',
      apiUrl: 'https://api.no-tickets.com',
      formatVersion: 1,
    });
  });

  it('includes .gitignore with exact content', () => {
    const plan = buildInitPlan('claude-code', TEST_DATE);
    const gitignore = plan.files.find((f) => f.path === '.notickets/.gitignore');

    expect(gitignore).toBeDefined();
    expect(gitignore!.content).toBe('config.json\n.last-push.json\n');
  });

  it('includes epic.md with correct frontmatter and sections', () => {
    const plan = buildInitPlan('claude-code', TEST_DATE);
    const epicFile = plan.files.find((f) => f.path === '.notickets/getting-started/epic.md');

    expect(epicFile).toBeDefined();
    expect(epicFile!.content).toContain('id: getting-started');
    expect(epicFile!.content).toContain('type: epic');
    expect(epicFile!.content).toContain('title: Getting Started');
    expect(epicFile!.content).toContain('status: not_started');
    expect(epicFile!.content).toContain(`created: ${TEST_DATE}`);
    expect(epicFile!.content).toContain(`updated: ${TEST_DATE}`);
    expect(epicFile!.content).toContain('## Goals');
    expect(epicFile!.content).toContain('## Features');
    expect(epicFile!.content).toContain('first-feature.md');
  });

  it('includes first-feature.md with correct frontmatter, criteria, and tasks', () => {
    const plan = buildInitPlan('claude-code', TEST_DATE);
    const featureFile = plan.files.find((f) => f.path === '.notickets/getting-started/first-feature.md');

    expect(featureFile).toBeDefined();
    expect(featureFile!.content).toContain('id: first-feature');
    expect(featureFile!.content).toContain('type: feature');
    expect(featureFile!.content).toContain('epic: getting-started');
    expect(featureFile!.content).toContain('phase: ideation');
    expect(featureFile!.content).toContain('status: not_started');
    expect(featureFile!.content).toContain(`created: ${TEST_DATE}`);
    expect(featureFile!.content).toContain('## Acceptance Criteria');
    expect(featureFile!.content).toContain('## Tasks');
    expect(featureFile!.content).toContain('### 1. First task');
  });

  it('uses injected date in all dated files', () => {
    const plan = buildInitPlan('claude-code', '2099-12-31');
    const epicFile = plan.files.find((f) => f.path.endsWith('epic.md'));
    const featureFile = plan.files.find((f) => f.path.endsWith('first-feature.md'));

    expect(epicFile!.content).toContain('created: 2099-12-31');
    expect(featureFile!.content).toContain('created: 2099-12-31');
  });

  it('includes skill install path for Claude Code', () => {
    expect(buildInitPlan('claude-code', TEST_DATE).skillInstallPath).toBe('.claude/skills/nt');
  });

  it('includes skill install path for Cursor', () => {
    expect(buildInitPlan('cursor', TEST_DATE).skillInstallPath).toBe('.cursor/skills/nt');
  });

  it('includes skill install path for Windsurf', () => {
    expect(buildInitPlan('windsurf', TEST_DATE).skillInstallPath).toBe('.windsurf/skills/nt');
  });

  it('returns undefined skill path for generic', () => {
    expect(buildInitPlan('generic', TEST_DATE).skillInstallPath).toBeUndefined();
  });

  it('sets tool in plan', () => {
    expect(buildInitPlan('claude-code', TEST_DATE).tool).toBe('claude-code');
    expect(buildInitPlan('generic', TEST_DATE).tool).toBe('generic');
  });
});
