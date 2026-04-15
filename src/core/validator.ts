import type { ParseResult, ParsedEpic, ParsedFeature, ValidationError, ValidationResult } from './types.js';
import { epicFrontmatterSchema, featureFrontmatterSchema, taskSchema } from './schemas.js';

/**
 * Validate a complete ParseResult against the no-tickets format spec.
 * Pure function — no I/O, no side effects.
 */
export function validate(parsed: ParseResult): ValidationResult {
  const errors: ValidationError[] = [];

  const epicIds = new Set(parsed.epics.map((e) => e.frontmatter.id));

  for (const epic of parsed.epics) {
    errors.push(...validateEpic(epic));
  }

  for (const feature of parsed.features) {
    errors.push(...validateFeature(feature, epicIds));
  }

  errors.push(...validateUniqueIds(parsed));

  return { valid: errors.length === 0, errors };
}

/**
 * Validate a single epic's frontmatter.
 */
function validateEpic(epic: ParsedEpic): readonly ValidationError[] {
  const errors: ValidationError[] = [];
  const result = epicFrontmatterSchema.safeParse(epic.frontmatter);

  if (!result.success) {
    for (const issue of result.error.issues) {
      errors.push({
        file: epic.filePath,
        field: issue.path.join('.'),
        message: issue.message,
        suggestion: formatSuggestion(issue.path.join('.'), issue.message),
      });
    }
  }

  return errors;
}

/**
 * Validate a single feature/fix's frontmatter, tasks, and epic reference.
 */
function validateFeature(
  feature: ParsedFeature,
  epicIds: ReadonlySet<string>,
): readonly ValidationError[] {
  const errors: ValidationError[] = [];

  // Validate frontmatter
  const result = featureFrontmatterSchema.safeParse(feature.frontmatter);
  const frontmatterValid = result.success;
  if (!result.success) {
    for (const issue of result.error.issues) {
      errors.push({
        file: feature.filePath,
        field: issue.path.join('.'),
        message: issue.message,
        suggestion: formatSuggestion(issue.path.join('.'), issue.message),
      });
    }
  }

  // Only check epic reference if frontmatter is valid (avoid duplicate/confusing errors)
  if (frontmatterValid && feature.frontmatter.epic && !epicIds.has(feature.frontmatter.epic)) {
    errors.push({
      file: feature.filePath,
      field: 'epic',
      message: `Epic "${feature.frontmatter.epic}" not found`,
      suggestion: `Create .notickets/${feature.frontmatter.epic}/epic.md or fix the epic reference`,
    });
  }

  // Validate tasks
  errors.push(...validateTasks(feature));

  return errors;
}

/**
 * Validate task numbering and status values within a feature.
 */
function validateTasks(feature: ParsedFeature): readonly ValidationError[] {
  const errors: ValidationError[] = [];
  const seen = new Set<number>();

  for (const task of feature.tasks) {
    // Validate task schema
    const result = taskSchema.safeParse(task);
    if (!result.success) {
      for (const issue of result.error.issues) {
        errors.push({
          file: feature.filePath,
          field: `task.${task.number}.${issue.path.join('.')}`,
          message: issue.message,
        });
      }
    }

    // Check for duplicate numbers
    if (seen.has(task.number)) {
      errors.push({
        file: feature.filePath,
        field: `task.${task.number}`,
        message: `Duplicate task number ${task.number}`,
        suggestion: `Renumber tasks sequentially starting from 1`,
      });
    }
    seen.add(task.number);
  }

  // Check for gaps in numbering
  if (feature.tasks.length > 0) {
    const numbers = feature.tasks.map((t) => t.number).sort((a, b) => a - b);
    for (let i = 0; i < numbers.length; i++) {
      const expected = i + 1;
      if (numbers[i] !== expected) {
        errors.push({
          file: feature.filePath,
          field: 'tasks',
          message: `Task numbering gap: expected ${expected}, found ${numbers[i]}`,
          suggestion: 'Renumber tasks sequentially starting from 1',
        });
        break;
      }
    }
  }

  return errors;
}

/**
 * Check for duplicate IDs across all documents.
 */
function validateUniqueIds(parsed: ParseResult): readonly ValidationError[] {
  const errors: ValidationError[] = [];
  const idMap = new Map<string, string>();

  for (const epic of parsed.epics) {
    const existing = idMap.get(epic.frontmatter.id);
    if (existing) {
      errors.push({
        file: epic.filePath,
        field: 'id',
        message: `Duplicate ID "${epic.frontmatter.id}" (also in ${existing})`,
      });
    }
    idMap.set(epic.frontmatter.id, epic.filePath);
  }

  for (const feature of parsed.features) {
    const existing = idMap.get(feature.frontmatter.id);
    if (existing) {
      errors.push({
        file: feature.filePath,
        field: 'id',
        message: `Duplicate ID "${feature.frontmatter.id}" (also in ${existing})`,
      });
    }
    idMap.set(feature.frontmatter.id, feature.filePath);
  }

  return errors;
}

/**
 * Format a human-readable suggestion for a validation error.
 */
function formatSuggestion(field: string, message: string): string {
  if (field === 'id') return 'Use lowercase letters, numbers, and hyphens (e.g., "my-feature-name")';
  if (field === 'phase') return 'Use one of: ideation, development, testing, review, done';
  if (field === 'status') return 'Use one of: not_started, in_progress, completed';
  if (field === 'type') return 'Use one of: epic, feature, fix';
  if (field === 'assignee_type') return 'Use one of: human, agent';
  if (message.includes('YYYY-MM-DD')) return 'Use ISO date format (e.g., "2026-04-05")';
  return '';
}
