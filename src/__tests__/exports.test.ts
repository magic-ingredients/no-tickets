import { describe, it, expect } from 'vitest';

// -- ./types subpath ----------------------------------------------------------

describe('@magic-ingredients/no-tickets/types subpath exports', () => {
  it('exports envelope types only (no push v2 types)', async () => {
    const types = await import('../core/types.js');
    const exported = Object.keys(types);

    // Push v2 types must be gone.
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
      expect(exported, `${banned} should not be exported from /types`).not.toContain(banned);
    }
  });
});

// -- ./schemas subpath --------------------------------------------------------

describe('@magic-ingredients/no-tickets/schemas subpath exports', () => {
  it('exports envelope zod schemas only (no push v2 schemas)', async () => {
    const schemas = await import('../core/schemas.js');
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
      expect(exported, `${banned} should not be exported from /schemas`).not.toContain(banned);
    }
  });

  it('re-exports envelope schemas via the /schemas subpath', async () => {
    const schemas = await import('../core/schemas.js');
    const exported = Object.keys(schemas);

    for (const expected of [
      'sourceSchema',
      'eventSchema',
      'subjectSchema',
      'subjectRefSchema',
      'interactionRequestSchema',
      'interactionResponseSchema',
      'interactionEventRefSchema',
      'TYPE_ID_REGEX',
    ]) {
      expect(exported, `${expected} should be re-exported from /schemas`).toContain(expected);
    }
  });
});

describe('envelope types subpath', () => {
  it('re-exports envelope types via the /types subpath', async () => {
    // Type-only exports don't appear in Object.keys at runtime, but they must
    // resolve at the type-checking layer. Importing the module compiles only
    // if the re-exports exist.
    const types = await import('../core/types.js');
    expect(types).toBeDefined();
  });
});

// -- package.json exports field -----------------------------------------------

describe('package.json exports field', () => {
  it('declares ./types and ./schemas subpath exports', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const exports = pkg.default.exports as Record<string, unknown>;

    expect(exports['./types']).toBeDefined();
    expect(exports['./schemas']).toBeDefined();
  });

  it('exports field has not regressed to including push paths', async () => {
    const pkg = await import('../../package.json', { with: { type: 'json' } });
    const exports = pkg.default.exports as Record<string, unknown>;
    const keys = Object.keys(exports);

    expect(keys).not.toContain('./push');
    expect(keys).not.toContain('./push-schemas');
  });
});
