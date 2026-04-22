import type { ParseResult, EpicState, FeatureState, StateSnapshot, Phase, ProjectEntity } from './types.js';

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
        completed: feature.tasks.reduce((n, t) => t.status === 'completed' ? n + 1 : n, 0),
      },
      tests: { total: 0, passing: 0 },
      meta: feature.frontmatter.meta,
    });

    epicMap.set(epicId, existing);
  }

  // Build epic states — only include features that belong to a known epic
  const epics: EpicState[] = parsed.epics.map((epic) => ({
    id: epic.frontmatter.id,
    title: epic.frontmatter.title,
    status: epic.frontmatter.status,
    features: epicMap.get(epic.frontmatter.id) ?? [],
  }));

  return {
    version: 1,
    epics,
    pushedAt: pushedAt ?? new Date().toISOString(),
  };
}

/**
 * Convert parsed .notickets/ documents into a flat ProjectEntity array
 * for the v2 Push payload's project schema.
 * Pure function — no I/O.
 */
export function toProjectEntities(parsed: ParseResult): readonly ProjectEntity[] {
  const epicIds = new Set(parsed.epics.map((e) => e.frontmatter.id));
  const entities: ProjectEntity[] = [];

  for (const epic of parsed.epics) {
    const epicEntity: ProjectEntity = {
      id: epic.frontmatter.id,
      type: 'epic',
      title: epic.frontmatter.title,
      status: epic.frontmatter.status,
      ...(epic.frontmatter.meta != null ? { meta: epic.frontmatter.meta } : {}),
    };
    entities.push(epicEntity);
  }

  for (const feature of parsed.features) {
    if (!epicIds.has(feature.frontmatter.epic)) continue;

    const featureEntity: ProjectEntity = {
      id: feature.frontmatter.id,
      type: 'feature',
      parentId: feature.frontmatter.epic,
      title: feature.frontmatter.title,
      status: feature.frontmatter.status,
      ...(feature.frontmatter.assignee != null ? { assignee: feature.frontmatter.assignee } : {}),
      ...(feature.frontmatter.assignee_type != null ? { assigneeType: feature.frontmatter.assignee_type } : {}),
      ...(feature.frontmatter.meta != null ? { meta: feature.frontmatter.meta } : {}),
    };
    entities.push(featureEntity);

    for (const task of feature.tasks) {
      const taskEntity: ProjectEntity = {
        id: `${feature.frontmatter.id}-task-${task.number}`,
        type: 'task',
        parentId: feature.frontmatter.id,
        title: task.title,
        status: task.status,
      };
      entities.push(taskEntity);
    }
  }

  return entities;
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
