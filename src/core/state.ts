import type { ParseResult, EpicState, FeatureState, StateSnapshot, Phase } from './types.js';

/**
 * Compute a state snapshot from parsed documents.
 * Pure function — accepts an optional timestamp to avoid reading the system clock.
 */
export function computeState(parsed: ParseResult, pushedAt?: string): StateSnapshot {
  const epicMap = new Map<string, FeatureState[]>();

  // Group features under their parent epic
  for (const feature of parsed.features) {
    const epicId = feature.frontmatter.epic;
    const existing = epicMap.get(epicId) ?? [];

    existing.push({
      id: feature.frontmatter.id,
      epicId,
      title: feature.frontmatter.title,
      type: feature.frontmatter.type,
      phase: feature.frontmatter.phase,
      assignee: feature.frontmatter.assignee,
      assigneeType: feature.frontmatter.assignee_type,
      tasks: {
        total: feature.tasks.length,
        completed: feature.tasks.filter((t) => t.status === 'completed').length,
      },
      tests: { total: 0, passing: 0 },
      meta: feature.frontmatter.meta,
    });

    epicMap.set(epicId, existing);
  }

  // Build epic states — only include features that belong to a known epic
  const epicIds = new Set(parsed.epics.map((e) => e.frontmatter.id));
  const epics: EpicState[] = parsed.epics.map((epic) => ({
    id: epic.frontmatter.id,
    title: epic.frontmatter.title,
    status: epic.frontmatter.status,
    features: epicMap.get(epic.frontmatter.id) ?? [],
  }));

  // Warn about orphan features (features referencing non-existent epics)
  const orphanFeatures = parsed.features.filter((f) => !epicIds.has(f.frontmatter.epic));
  if (orphanFeatures.length > 0) {
    // Orphan features are silently dropped — validation should catch this upstream
  }

  return {
    version: 1,
    epics,
    pushedAt: pushedAt ?? new Date().toISOString(),
  };
}

/**
 * Compute overall progress percentage for a state snapshot.
 * Pure function. Clamps result to 0-100.
 */
export function computeOverallProgress(snapshot: StateSnapshot): number {
  let totalTasks = 0;
  let completedTasks = 0;

  for (const epic of snapshot.epics) {
    for (const feature of epic.features) {
      totalTasks += feature.tasks.total;
      completedTasks += feature.tasks.completed;
    }
  }

  if (totalTasks === 0) return 0;
  return Math.min(100, Math.round((completedTasks / totalTasks) * 100));
}

/**
 * Compute progress for a single feature.
 * Pure function. Clamps result to 0-100.
 */
export function computeFeatureProgress(feature: FeatureState): number {
  const phaseProgress: Record<Phase, number> = {
    ideation: 0,
    development: 25,
    testing: 50,
    review: 75,
    done: 100,
  };

  const phasePercent = phaseProgress[feature.phase];
  const taskPercent = feature.tasks.total > 0
    ? (feature.tasks.completed / feature.tasks.total) * 100
    : 0;
  const testPercent = feature.tests.total > 0
    ? (feature.tests.passing / feature.tests.total) * 100
    : 0;

  const raw = feature.tests.total === 0
    ? phasePercent * 0.4 + taskPercent * 0.6
    : phasePercent * 0.3 + taskPercent * 0.35 + testPercent * 0.35;

  return Math.min(100, Math.round(raw));
}
