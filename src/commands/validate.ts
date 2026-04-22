import { parseFiles } from '../core/parser.js';
import { validate } from '../core/validator.js';
import type { FileEntry, ValidationResult } from '../core/types.js';

/**
 * Validate .notickets/ files against the format spec.
 * Pure function — accepts file contents, no I/O.
 */
export function validateFiles(files: readonly FileEntry[]): ValidationResult {
  const parsed = parseFiles(files);
  return validate(parsed);
}
