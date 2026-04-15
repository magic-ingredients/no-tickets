// Core library — pure, stateless functions. No I/O, no side effects.
// Only export tested modules.

export {
  parseFrontmatter,
  parseTasks,
  parseFiles,
  assembleEpic,
  assembleFeature,
  extractSection,
  parseGoals,
  parseAcceptanceCriteria,
  parseDescription,
} from './parser.js';

export { validate } from './validator.js';
export { computeState, computeOverallProgress, computeFeatureProgress } from './state.js';
export { computeDiff } from './diff.js';

// Schemas are available via '@magic-ingredients/no-tickets-client/schemas'
// subpath export. Not re-exported from core to keep the public API
// limited to tested functions.
