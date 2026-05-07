import { describe, it, expect } from 'vitest';
import { sourceFromTransport } from './source-from-transport.js';

describe('sourceFromTransport', () => {
  it('produces { name: "mcp", sdkVersion, attributes: { client: "unknown" } } when no hints supplied', () => {
    const source = sourceFromTransport({});

    expect(source.name).toBe('mcp');
    expect(typeof source.sdkVersion).toBe('string');
    expect(source.sdkVersion.length).toBeGreaterThan(0);
    expect(source.attributes).toEqual({ client: 'unknown' });
  });

  it('uses transport-supplied client when present', () => {
    const source = sourceFromTransport({ client: 'claude-code' });

    expect(source.attributes).toEqual({ client: 'claude-code' });
  });

  it('includes clientVersion when both client and clientVersion are provided', () => {
    const source = sourceFromTransport({
      client: 'claude-code',
      clientVersion: '1.2.3',
    });

    expect(source.attributes).toEqual({
      client: 'claude-code',
      clientVersion: '1.2.3',
    });
  });

  it('does NOT add clientVersion when client is missing (no orphaned version)', () => {
    const source = sourceFromTransport({ clientVersion: '1.2.3' });

    expect(source.attributes).toEqual({ client: 'unknown' });
  });

  it('treats an empty client string as missing', () => {
    const source = sourceFromTransport({ client: '' });

    expect(source.attributes).toEqual({ client: 'unknown' });
  });
});
