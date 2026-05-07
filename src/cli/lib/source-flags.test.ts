import { describe, it, expect } from 'vitest';
import { parseSourceFlags } from './source-flags.js';

describe('parseSourceFlags', () => {
  it('returns undefined when no source flags are provided', () => {
    expect(parseSourceFlags({})).toBeUndefined();
  });

  it('parses --source-name into source.name', () => {
    expect(parseSourceFlags({ name: 'my-tool' })).toEqual({ name: 'my-tool' });
  });

  it('parses a single --source-attribute into source.attributes', () => {
    expect(parseSourceFlags({ attributes: ['env=prod'] })).toEqual({
      attributes: { env: 'prod' },
    });
  });

  it('parses repeatable --source-attribute entries', () => {
    expect(
      parseSourceFlags({ attributes: ['env=prod', 'region=eu-west-1'] }),
    ).toEqual({
      attributes: { env: 'prod', region: 'eu-west-1' },
    });
  });

  it('combines name and attributes into one source object', () => {
    expect(
      parseSourceFlags({ name: 'tiny-brain', attributes: ['version=1.2.3'] }),
    ).toEqual({
      name: 'tiny-brain',
      attributes: { version: '1.2.3' },
    });
  });

  it('keeps the FIRST occurrence when the same attribute key appears twice (or last wins — pin one)', () => {
    // The test pins the contract: last value wins, matching command-line
    // override conventions where later flags supersede earlier ones.
    expect(parseSourceFlags({ attributes: ['env=staging', 'env=prod'] })).toEqual({
      attributes: { env: 'prod' },
    });
  });

  it('throws on a malformed attribute (no "=" separator)', () => {
    expect(() => parseSourceFlags({ attributes: ['no-equals'] })).toThrow(/key=value/);
  });

  it('throws on an empty attribute key', () => {
    expect(() => parseSourceFlags({ attributes: ['=val'] })).toThrow(/key/i);
  });

  it('preserves an empty value (key with nothing after the "=")', () => {
    // "key=" is a deliberate empty-value override; preserve it.
    expect(parseSourceFlags({ attributes: ['env='] })).toEqual({
      attributes: { env: '' },
    });
  });

  it('preserves "=" inside the value (only the first "=" splits)', () => {
    expect(parseSourceFlags({ attributes: ['url=https://x.com?a=1'] })).toEqual({
      attributes: { url: 'https://x.com?a=1' },
    });
  });
});
