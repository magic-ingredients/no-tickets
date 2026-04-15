import type { StateSnapshot, FeatureState, FeatureUpdate, SyncDelta } from './types.js';

/**
 * Compute the delta between two state snapshots.
 * Pure function — no I/O, no side effects.
 *
 * @param previous - The last pushed state (undefined on first push)
 * @param current - The current state to push
 * @returns A minimal delta describing what changed
 */
export function computeDiff(
  previous: StateSnapshot | undefined,
  current: StateSnapshot,
): SyncDelta {
  if (!previous) {
    const allFeatures = current.epics.flatMap((e) => e.features);
    return {
      added: allFeatures,
      updated: [],
      removed: [],
      isEmpty: allFeatures.length === 0,
    };
  }

  const prevFeatures = indexFeatures(previous);
  const currFeatures = indexFeatures(current);

  const added: FeatureState[] = [];
  const updated: FeatureUpdate[] = [];
  const removed: string[] = [];

  // Find added and updated
  for (const [id, feature] of currFeatures) {
    const prev = prevFeatures.get(id);
    if (!prev) {
      added.push(feature);
    } else {
      const changes = diffFeature(prev, feature);
      if (Object.keys(changes).length > 0) {
        updated.push({ id, changes });
      }
    }
  }

  // Find removed
  for (const id of prevFeatures.keys()) {
    if (!currFeatures.has(id)) {
      removed.push(id);
    }
  }

  return {
    added,
    updated,
    removed,
    isEmpty: added.length === 0 && updated.length === 0 && removed.length === 0,
  };
}

/**
 * Flatten all features from a snapshot into a map keyed by ID.
 */
function indexFeatures(snapshot: StateSnapshot): Map<string, FeatureState> {
  const map = new Map<string, FeatureState>();
  for (const epic of snapshot.epics) {
    for (const feature of epic.features) {
      map.set(feature.id, feature);
    }
  }
  return map;
}

/**
 * Compare two feature states and return changed fields.
 */
function diffFeature(
  prev: FeatureState,
  curr: FeatureState,
): Record<string, { from: unknown; to: unknown }> {
  const changes: Record<string, { from: unknown; to: unknown }> = {};

  if (prev.title !== curr.title) {
    changes['title'] = { from: prev.title, to: curr.title };
  }
  if (prev.type !== curr.type) {
    changes['type'] = { from: prev.type, to: curr.type };
  }
  if (prev.phase !== curr.phase) {
    changes['phase'] = { from: prev.phase, to: curr.phase };
  }
  if (prev.assignee !== curr.assignee) {
    changes['assignee'] = { from: prev.assignee, to: curr.assignee };
  }
  if (prev.assigneeType !== curr.assigneeType) {
    changes['assigneeType'] = { from: prev.assigneeType, to: curr.assigneeType };
  }
  if (prev.tasks.completed !== curr.tasks.completed || prev.tasks.total !== curr.tasks.total) {
    changes['tasks'] = { from: prev.tasks, to: curr.tasks };
  }
  if (prev.tests.passing !== curr.tests.passing || prev.tests.total !== curr.tests.total) {
    changes['tests'] = { from: prev.tests, to: curr.tests };
  }
  if (JSON.stringify(prev.meta) !== JSON.stringify(curr.meta)) {
    changes['meta'] = { from: prev.meta, to: curr.meta };
  }

  return changes;
}
