import { describe, it, expect } from 'vitest';
import { sourceSchema, mergeSource, SDK_VERSION } from '../source.js';

// -- Schema shape -------------------------------------------------------------

describe('sourceSchema', () => {
  it('accepts the minimal valid shape (name + sdkVersion only)', () => {
    const parsed = sourceSchema.parse({ name: 'cli', sdkVersion: '1.2.3' });
    expect(parsed).toEqual({ name: 'cli', sdkVersion: '1.2.3' });
  });

  it('accepts the fully-populated shape', () => {
    const input = {
      name: 'integration',
      sdkVersion: '1.2.3',
      version: '0.4.2',
      attributes: { integration: 'tiny-brain', machine: 'a1b2c3', runId: 42, debug: true },
    };
    expect(sourceSchema.parse(input)).toEqual(input);
  });

  it('rejects missing name', () => {
    expect(() => sourceSchema.parse({ sdkVersion: '1.2.3' })).toThrow();
  });

  it('rejects missing sdkVersion', () => {
    expect(() => sourceSchema.parse({ name: 'cli' })).toThrow();
  });

  it('rejects non-string name', () => {
    expect(() => sourceSchema.parse({ name: 42, sdkVersion: '1.2.3' })).toThrow();
  });

  it('rejects non-string sdkVersion', () => {
    expect(() => sourceSchema.parse({ name: 'cli', sdkVersion: 123 })).toThrow();
  });

  it('rejects empty string for name', () => {
    expect(() => sourceSchema.parse({ name: '', sdkVersion: '1.2.3' })).toThrow();
  });

  it('rejects empty string for sdkVersion', () => {
    expect(() => sourceSchema.parse({ name: 'cli', sdkVersion: '' })).toThrow();
  });

  it('accepts strings, numbers, and booleans in attributes', () => {
    const input = {
      name: 'ci',
      sdkVersion: '1.2.3',
      attributes: { provider: 'github', runId: 12345, debug: false },
    };
    expect(sourceSchema.parse(input)).toEqual(input);
  });

  it('rejects nested objects in attributes', () => {
    expect(() =>
      sourceSchema.parse({
        name: 'cli',
        sdkVersion: '1.2.3',
        attributes: { nested: { wrong: 'shape' } },
      }),
    ).toThrow();
  });

  it('rejects null in attributes', () => {
    expect(() =>
      sourceSchema.parse({
        name: 'cli',
        sdkVersion: '1.2.3',
        attributes: { thing: null },
      }),
    ).toThrow();
  });

  it('rejects arrays in attributes', () => {
    expect(() =>
      sourceSchema.parse({
        name: 'cli',
        sdkVersion: '1.2.3',
        attributes: { tags: ['a', 'b'] },
      }),
    ).toThrow();
  });
});

// -- mergeSource semantics ----------------------------------------------------

describe('mergeSource', () => {
  const auto = {
    name: 'cli',
    sdkVersion: '1.2.3',
    version: '1.2.3',
    attributes: { machine: 'hostA', provider: 'github' },
  } as const;

  it('returns auto unchanged when override is undefined', () => {
    expect(mergeSource(auto, undefined)).toEqual(auto);
  });

  it('returns auto unchanged when override is empty object', () => {
    expect(mergeSource(auto, {})).toEqual(auto);
  });

  it('overrides name when caller provides it', () => {
    const result = mergeSource(auto, { name: 'integration' });
    expect(result.name).toBe('integration');
    expect(result.sdkVersion).toBe('1.2.3');
  });

  it('overrides version when caller provides it', () => {
    const result = mergeSource(auto, { version: '0.4.2' });
    expect(result.version).toBe('0.4.2');
    expect(result.name).toBe('cli');
  });

  it('overrides sdkVersion when caller provides it (rare but allowed)', () => {
    const result = mergeSource(auto, { sdkVersion: '9.9.9' });
    expect(result.sdkVersion).toBe('9.9.9');
  });

  it('merges attributes per-key, with override winning conflicts', () => {
    const result = mergeSource(auto, {
      attributes: { machine: 'hostB', feature: 'experimental' },
    });
    expect(result.attributes).toEqual({
      machine: 'hostB', // override wins
      provider: 'github', // preserved from auto
      feature: 'experimental', // added by override
    });
  });

  it('preserves auto.attributes when override has no attributes', () => {
    const result = mergeSource(auto, { name: 'integration' });
    expect(result.attributes).toEqual({ machine: 'hostA', provider: 'github' });
  });

  it('returns auto.attributes when both have undefined attributes', () => {
    const autoNoAttrs = { name: 'cli', sdkVersion: '1.2.3' };
    const result = mergeSource(autoNoAttrs, { name: 'integration' });
    expect(result.attributes).toBeUndefined();
  });

  it('uses override.attributes when auto has none', () => {
    const autoNoAttrs = { name: 'cli', sdkVersion: '1.2.3' };
    const result = mergeSource(autoNoAttrs, { attributes: { feature: 'x' } });
    expect(result.attributes).toEqual({ feature: 'x' });
  });

  it('result conforms to sourceSchema', () => {
    const result = mergeSource(auto, { name: 'integration', attributes: { feature: 'x' } });
    expect(() => sourceSchema.parse(result)).not.toThrow();
  });
});

// -- SDK_VERSION constant -----------------------------------------------------

describe('SDK_VERSION', () => {
  it('is a non-empty string', () => {
    expect(typeof SDK_VERSION).toBe('string');
    expect(SDK_VERSION.length).toBeGreaterThan(0);
  });

  it('matches semver pattern (major.minor.patch with optional prerelease)', () => {
    expect(SDK_VERSION).toMatch(/^\d+\.\d+\.\d+(-[a-z0-9.-]+)?$/i);
  });

  it('matches the version in package.json (resolved at build time)', async () => {
    const pkg = await import('../../../package.json', { with: { type: 'json' } });
    expect(SDK_VERSION).toBe(pkg.default.version);
  });
});
