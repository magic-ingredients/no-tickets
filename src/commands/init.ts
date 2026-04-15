type LlmTool = 'claude-code' | 'cursor' | 'windsurf' | 'generic';

interface FileEntry {
  readonly path: string;
  readonly content: string;
}

interface InitPlan {
  readonly tool: LlmTool;
  readonly directories: readonly string[];
  readonly files: readonly FileEntry[];
  readonly skillInstallPath?: string;
}

const TOOL_DETECTORS: readonly { readonly dir: string; readonly tool: LlmTool }[] = [
  { dir: '.claude', tool: 'claude-code' },
  { dir: '.cursor', tool: 'cursor' },
  { dir: '.windsurf', tool: 'windsurf' },
];

const SKILL_PATHS: Record<LlmTool, string | undefined> = {
  'claude-code': '.claude/skills/nt',
  'cursor': '.cursor/skills/nt',
  'windsurf': '.windsurf/skills/nt',
  'generic': undefined,
};

const EXAMPLE_EPIC_ID = 'getting-started';

/**
 * Detect which LLM tool is present based on directory entries.
 * Pure function.
 */
function detectLlmTool(entries: readonly string[]): LlmTool {
  const entrySet = new Set(entries);
  for (const detector of TOOL_DETECTORS) {
    if (entrySet.has(detector.dir)) {
      return detector.tool;
    }
  }
  return 'generic';
}

/**
 * Build an init plan describing what directories and files to create.
 * Pure function — accepts date to avoid reading system clock.
 */
function buildInitPlan(tool: LlmTool, date?: string): InitPlan {
  const today = date ?? new Date().toISOString().split('T')[0]!;

  const directories = [
    '.notickets',
    `.notickets/${EXAMPLE_EPIC_ID}`,
  ];

  const files: FileEntry[] = [
    {
      path: '.notickets/config.example.json',
      content: JSON.stringify({
        teamId: 'your-team-id',
        projectId: 'your-project-id',
        apiUrl: 'https://api.no-tickets.com',
        formatVersion: 1,
      }, null, 2) + '\n',
    },
    {
      path: '.notickets/.gitignore',
      content: 'config.json\n.last-push.json\n',
    },
    {
      path: `.notickets/${EXAMPLE_EPIC_ID}/epic.md`,
      content: `---\nid: ${EXAMPLE_EPIC_ID}\ntype: epic\ntitle: Getting Started\nstatus: not_started\ncreated: ${today}\nupdated: ${today}\n---\n\n# Getting Started\n\nYour first epic. Replace this with your actual project goals.\n\n## Goals\n\n- Set up project structure\n- Define initial features\n- Start building\n\n## Features\n\n- [first-feature.md](first-feature.md) — Your first feature\n`,
    },
    {
      path: `.notickets/${EXAMPLE_EPIC_ID}/first-feature.md`,
      content: `---\nid: first-feature\ntype: feature\nepic: ${EXAMPLE_EPIC_ID}\ntitle: First Feature\nphase: ideation\nstatus: not_started\ncreated: ${today}\nupdated: ${today}\n---\n\n# First Feature\n\nDescribe what this feature does.\n\n## Acceptance Criteria\n\n- [ ] First criterion\n- [ ] Second criterion\n\n## Tasks\n\n### 1. First task\nstatus: not_started\n\nDescribe what needs to be done.\n`,
    },
  ];

  return {
    tool,
    directories,
    files,
    skillInstallPath: SKILL_PATHS[tool],
  };
}

export { detectLlmTool, buildInitPlan };
export type { LlmTool, InitPlan };
