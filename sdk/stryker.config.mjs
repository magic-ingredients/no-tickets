// @ts-check
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { createRequire } from 'module';

const require = createRequire(import.meta.url);
const vitestRunnerPath = dirname(require.resolve('@stryker-mutator/vitest-runner/package.json'));

/** @type {import('@stryker-mutator/api/core').PartialStrykerOptions} */
export default {
  testRunner: 'vitest',
  plugins: [join(vitestRunnerPath, 'dist/src/index.js')],
  mutate: ['src/**/*.ts', '!src/**/*.test.ts', '!src/**/__tests__/**'],
  reporters: ['clear-text', 'json'],
  jsonReporter: { fileName: 'reports/mutation/stryker-report.json' },
  coverageAnalysis: 'perTest',
  ignorePatterns: ['coverage/**', 'dist/**', '.stryker-tmp/**', 'reports/**'],
};
