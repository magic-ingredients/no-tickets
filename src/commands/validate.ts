import { parseFiles } from '../core/parser.js';
import { validate } from '../core/validator.js';
import type { FileEntry, ValidationResult } from '../core/types.js';

/**
 * Validate .notickets/ files against the format spec.
 * Files with unrecognized or missing type fields are silently skipped
 * (parseFiles only processes epic/feature/fix types).
 */
export function validateFiles(files: readonly FileEntry[]): ValidationResult {
  const parsed = parseFiles(files);
  return validate(parsed);
}
