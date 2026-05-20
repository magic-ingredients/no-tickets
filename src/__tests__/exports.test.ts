import { describe, it, expect } from 'vitest';
import * as types from '../core/types.js';
import * as schemas from '../core/schemas.js';
import type { Source, Event, TypeIdParts } from '../core/types.js';

// -- ./schemas subpath --------------------------------------------------------

const EXPECTED_SCHEMAS_KEYS = [
  // v1 frontmatter
  'phaseSchema',
  'entityStatusSchema',
  'taskStatusSchema',
  'assigneeTypeSchema',
  'epicFrontmatterSchema',
  'featureFrontmatterSchema',
  'taskSchema',
  // envelope
  'sourceSchema',
  'mergeSource',
  'SDK_VERSION',
  'eventSchema',
  'TYPE_ID_REGEX',
  'parseTypeId',
  'formatTypeId',
  'isTypeId',
] as const;

describe('@magic-ingredients/no-tickets/schemas subpath', () => {
  it('exports exactly the expected surface (allow-list, fails on add or remove)', () => {
    const actual = Object.keys(schemas).sort();
    const expected = [...EXPECTED_SCHEMAS_KEYS].sort();
    expect(actual).toEqual(expected);
  });

  it('does not export push v2 schemas (regression guard)', () => {
    const exported = Object.keys(schemas);
    for (const banned of [
      'pushSchema',
      'workEntityTypeSchema',
      'engineeringPhaseSchema',
      'acceptanceStatusSchema',
      'prioritySchema',
      'codeQualitySourceSchema',
      'pushEnvironmentSchema',
      'sessionSchema',
      'workEntitySchema',
      'workDataSchema',
      'engineeringReviewSchema',
      'engineeringTaskSchema',
      'engineeringDataSchema',
      'productUpdateSchema',
      'productDataSchema',
      'codeQualityDataSchema',
      'documentTypeSchema',
    ]) {
      expect(exported, `${banned} must not appear`).not.toContain(banned);
    }
  });
});

// -- ./types subpath ----------------------------------------------------------

describe('@magic-ingredients/no-tickets/types subpath', () => {
  it('does not export push v2 types (runtime regression guard)', () => {
    const exported = Object.keys(types);
    for (const banned of [
      'Push',
      'PushResult',
      'WorkSchema',
      'WorkEntity',
      'WorkEntityType',
      'EngineeringSchema',
      'EngineeringTask',
      'EngineeringReview',
      'EngineeringPhase',
      'ProductSchema',
      'ProductUpdate',
      'AcceptanceStatus',
      'Priority',
      'CodeQualitySchema',
      'CodeQualitySource',
      'Session',
      'PushEnvironment',
      'BoardState',
      'BoardColumn',
      'FeedEvent',
      'SessionState',
    ]) {
      expect(exported, `${banned} must not appear`).not.toContain(banned);
    }
  });

  it('re-exports envelope types (compile-time check via typed assignments)', () => {
    // Each binding below fails to compile if the corresponding `export type`
    // is removed from src/core/types.ts. tsc-as-test is the actual assertion;
    // the runtime check below just keeps the bindings live.
    const source: Source = { name: 'cli', sdkVersion: '0.0.0' };
    const event: Event<{ x: number }> = { type: 'a.b.c.v1', data: { x: 1 }, source };
    const typeIdParts: TypeIdParts = { domain: 'a', entity: 'b', action: 'c', version: 1 };

    expect([source, event, typeIdParts]).toHaveLength(3);
  });
});

// -- package.json exports field -----------------------------------------------

describe('package.json exports field', () => {
  it('declares ./types pointing at dist/core/types.js with d.ts companion', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const exports = pkg.default.exports as Record<string, { types: string; import: string }>;

    expect(exports['./types']).toEqual({
      types: './dist/core/types.d.ts',
      import: './dist/core/types.js',
    });
  });

  it('declares ./schemas pointing at dist/core/schemas.js with d.ts companion', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const exports = pkg.default.exports as Record<string, { types: string; import: string }>;

    expect(exports['./schemas']).toEqual({
      types: './dist/core/schemas.d.ts',
      import: './dist/core/schemas.js',
    });
  });

  it('declares root . pointing at dist/core/index.js with d.ts companion', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const exports = pkg.default.exports as Record<string, { types: string; import: string }>;

    expect(exports['.']).toEqual({
      types: './dist/core/index.d.ts',
      import: './dist/core/index.js',
    });
  });

  it('does not regress to including push subpaths', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const exports = pkg.default.exports as Record<string, unknown>;
    const keys = Object.keys(exports);

    expect(keys).not.toContain('./push');
    expect(keys).not.toContain('./push-schemas');
  });

  it('does not declare a `bin` field (CLI retired in favour of native nt binary)', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    expect(pkg.default).not.toHaveProperty('bin');
  });

  it('does not depend on the MCP SDK (MCP retired in favour of nt-mcp Rust binary)', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const deps = (pkg.default.dependencies ?? {}) as Record<string, string>;
    const devDeps = (pkg.default.devDependencies ?? {}) as Record<string, string>;
    expect(deps).not.toHaveProperty('@modelcontextprotocol/sdk');
    expect(devDeps).not.toHaveProperty('@modelcontextprotocol/sdk');
  });

  it('does not ship a bin/ directory via the files allowlist', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const files = (pkg.default.files ?? []) as readonly string[];
    expect(files).not.toContain('bin');
  });
});
