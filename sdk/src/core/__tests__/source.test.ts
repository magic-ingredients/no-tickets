import { describe, it, expect } from 'vitest';
import { sourceSchema, mergeSource, SDK_VERSION, type Source } from '../source.js';

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

  it('tolerates unknown top-level keys (strips them, does not throw)', () => {
    const parsed = sourceSchema.parse({
      name: 'cli',
      sdkVersion: '1.2.3',
      unknownField: 'allowed for forward-compat',
    });
    expect(parsed).toEqual({ name: 'cli', sdkVersion: '1.2.3' });
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
  const auto: Source = {
    name: 'cli',
    sdkVersion: '1.2.3',
    version: '1.2.3',
    attributes: { machine: 'hostA', provider: 'github' },
  };

  it('returns a fresh object when override is undefined (does not leak auto reference)', () => {
    const result = mergeSource(auto, undefined);
    expect(result).toEqual(auto);
    expect(result).not.toBe(auto);
  });

  it('returns a fresh object when override is empty', () => {
    const result = mergeSource(auto, {});
    expect(result).toEqual(auto);
    expect(result).not.toBe(auto);
  });

  it('overrides name when caller provides a non-empty value', () => {
    const result = mergeSource(auto, { name: 'integration' });
    expect(result.name).toBe('integration');
    expect(result.sdkVersion).toBe('1.2.3');
  });

  it('overrides version when caller provides a non-empty value', () => {
    const result = mergeSource(auto, { version: '0.4.2' });
    expect(result.version).toBe('0.4.2');
    expect(result.name).toBe('cli');
  });

  it('overrides sdkVersion when caller provides a non-empty value', () => {
    const result = mergeSource(auto, { sdkVersion: '9.9.9' });
    expect(result.sdkVersion).toBe('9.9.9');
  });

  it('treats empty-string name override as a gap (falls back to auto)', () => {
    const result = mergeSource(auto, { name: '' });
    expect(result.name).toBe('cli');
  });

  it('treats empty-string sdkVersion override as a gap (falls back to auto)', () => {
    const result = mergeSource(auto, { sdkVersion: '' });
    expect(result.sdkVersion).toBe('1.2.3');
  });

  it('treats empty-string version override as a gap (falls back to auto)', () => {
    const result = mergeSource(auto, { version: '' });
    expect(result.version).toBe('1.2.3');
  });

  it('merges attributes per-key, with override winning conflicts', () => {
    const result = mergeSource(auto, {
      attributes: { machine: 'hostB', feature: 'experimental' },
    });
    expect(result.attributes).toEqual({
      machine: 'hostB',
      provider: 'github',
      feature: 'experimental',
    });
  });

  it('preserves auto.attributes when override has no attributes key', () => {
    const result = mergeSource(auto, { name: 'integration' });
    expect(result.attributes).toEqual({ machine: 'hostA', provider: 'github' });
  });

  it('treats explicit-undefined attributes override as a gap (preserves auto.attributes)', () => {
    const result = mergeSource(auto, { attributes: undefined });
    expect(result.attributes).toEqual({ machine: 'hostA', provider: 'github' });
  });

  it('omits the attributes key entirely when both auto and override have none', () => {
    const autoNoAttrs: Source = { name: 'cli', sdkVersion: '1.2.3' };
    const result = mergeSource(autoNoAttrs, { name: 'integration' });
    expect(result.attributes).toBeUndefined();
    expect(Object.keys(result)).not.toContain('attributes');
  });

  it('omits the version key entirely when neither auto nor override has it', () => {
    const autoNoVersion: Source = { name: 'cli', sdkVersion: '1.2.3' };
    const result = mergeSource(autoNoVersion, { name: 'integration' });
    expect(result.version).toBeUndefined();
    expect(Object.keys(result)).not.toContain('version');
  });

  it('uses override.attributes when auto has none', () => {
    const autoNoAttrs: Source = { name: 'cli', sdkVersion: '1.2.3' };
    const result = mergeSource(autoNoAttrs, { attributes: { feature: 'x' } });
    expect(result.attributes).toEqual({ feature: 'x' });
  });

  describe('result conforms to sourceSchema', () => {
    const cases: Array<[string, Partial<Source> | undefined]> = [
      ['undefined override', undefined],
      ['empty override', {}],
      ['name-only override', { name: 'integration' }],
      ['attributes-only override', { attributes: { feature: 'experimental' } }],
      ['attributes with all primitive types', { attributes: { s: 'x', n: 1, b: true } }],
      ['empty-string name (falls back to auto)', { name: '' }],
      ['version override', { version: '0.4.2' }],
    ];

    for (const [label, override] of cases) {
      it(label, () => {
        const result = mergeSource(auto, override);
        expect(() => sourceSchema.parse(result)).not.toThrow();
      });
    }
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
});
