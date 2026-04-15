export const VALID_STATUSES = ['not_started', 'in_progress', 'completed'] as const;
export const VALID_PHASES = ['red', 'green', 'refactor', 'review', 'complete'] as const;

export type TaskStatus = typeof VALID_STATUSES[number];
export type TaskPhase = typeof VALID_PHASES[number];

export interface TaskUpdateData {
  readonly taskId: string;
  readonly status: TaskStatus;
  readonly phase?: TaskPhase;
  readonly commitSha?: string;
  readonly coverage?: number;
  readonly testsTotal?: number;
  readonly testsPassing?: number;
  readonly reviewVerdict?: string;
  readonly documentation?: boolean;
}

type ParseResult =
  | { readonly success: true; readonly data: TaskUpdateData }
  | { readonly success: false; readonly error: string };

function getFlag(args: readonly string[], name: string): string | undefined {
  const index = args.indexOf(`--${name}`);
  if (index === -1) return undefined;
  const value = args[index + 1];
  if (value === undefined || value.startsWith('--')) return undefined;
  return value;
}

function hasFlag(args: readonly string[], name: string): boolean {
  return args.includes(`--${name}`);
}

/**
 * Parse CLI arguments for `no-tickets task update`.
 * Pure function — no I/O.
 */
export function parseTaskUpdateArgs(args: readonly string[]): ParseResult {
  const taskId = getFlag(args, 'task');
  if (!taskId) {
    return { success: false, error: 'Missing required flag: --task <id>' };
  }

  const statusRaw = getFlag(args, 'status');
  if (!statusRaw) {
    return { success: false, error: 'Missing required flag: --status <status>' };
  }

  if (!VALID_STATUSES.includes(statusRaw as TaskStatus)) {
    return {
      success: false,
      error: `Invalid status "${statusRaw}". Must be one of: ${VALID_STATUSES.join(', ')}`,
    };
  }
  const status = statusRaw as TaskStatus;

  const phaseRaw = getFlag(args, 'phase');
  if (phaseRaw !== undefined && !VALID_PHASES.includes(phaseRaw as TaskPhase)) {
    return {
      success: false,
      error: `Invalid phase "${phaseRaw}". Must be one of: ${VALID_PHASES.join(', ')}`,
    };
  }
  const phase = phaseRaw as TaskPhase | undefined;

  const commitSha = getFlag(args, 'commit-sha');

  const coverageRaw = getFlag(args, 'coverage');
  let coverage: number | undefined;
  if (coverageRaw !== undefined) {
    coverage = Number(coverageRaw);
    if (isNaN(coverage) || coverage < 0 || coverage > 100) {
      return { success: false, error: 'Invalid coverage: must be a number between 0 and 100' };
    }
  }

  const testsTotalRaw = getFlag(args, 'tests-total');
  let testsTotal: number | undefined;
  if (testsTotalRaw !== undefined) {
    testsTotal = Number(testsTotalRaw);
    if (isNaN(testsTotal) || testsTotal < 0 || !Number.isInteger(testsTotal)) {
      return { success: false, error: 'Invalid tests-total: must be a non-negative integer' };
    }
  }

  const testsPassingRaw = getFlag(args, 'tests-passing');
  let testsPassing: number | undefined;
  if (testsPassingRaw !== undefined) {
    testsPassing = Number(testsPassingRaw);
    if (isNaN(testsPassing) || testsPassing < 0 || !Number.isInteger(testsPassing)) {
      return { success: false, error: 'Invalid tests-passing: must be a non-negative integer' };
    }
  }

  const reviewVerdict = getFlag(args, 'review-verdict');
  const documentation = hasFlag(args, 'documentation') ? true : undefined;

  return {
    success: true,
    data: {
      taskId,
      status,
      ...(phase !== undefined ? { phase } : {}),
      ...(commitSha !== undefined ? { commitSha } : {}),
      ...(coverage !== undefined ? { coverage } : {}),
      ...(testsTotal !== undefined ? { testsTotal } : {}),
      ...(testsPassing !== undefined ? { testsPassing } : {}),
      ...(reviewVerdict !== undefined ? { reviewVerdict } : {}),
      ...(documentation !== undefined ? { documentation } : {}),
    },
  };
}
