import { describe, it, expect } from 'vitest';
import { maskToken } from './config-io.js';

describe('maskToken', () => {
  it('keeps the nt_push_ prefix and the last 4 chars of a real token', () => {
    expect(maskToken('nt_push_a0e79856da36a60367c38def8ccac62e85b79d81a46863338b21fe86f29ae0c9')).toBe(
      'nt_push_…e0c9',
    );
  });

  it('falls back to first-4 / last-2 for tokens without the nt_push_ prefix', () => {
    // Defensive: maskToken should never echo the full secret regardless of
    // input format. A non-nt_push_ token (legacy or future format) still
    // gets masked.
    expect(maskToken('legacy_xxxxxxxxxxxxxxxxxxxxxxxx')).toBe('lega…xx');
  });

  it('returns the input unchanged when too short to mask meaningfully (<= 6 chars)', () => {
    // Edge case — tokens this short are unrealistic but the function
    // shouldn't crash or produce nonsense like a "…" with no prefix.
    expect(maskToken('abc')).toBe('abc');
    expect(maskToken('abcdef')).toBe('abcdef');
  });

  it('masks a 7-char unknown-format token via the fallback', () => {
    // Just over the unmaskable threshold; takes the prefix-suffix path.
    expect(maskToken('abcdefg')).toBe('abcd…fg');
  });

  it('masks an empty token (defensive — never crashes on malformed input)', () => {
    expect(maskToken('')).toBe('');
  });
});
