import { describe, it, expect } from 'vitest';
import { fuzzyMatch } from './fuzzy-match.js';

describe('fuzzyMatch', () => {
  it('returns the candidate first when it is an exact prefix of one entry', () => {
    const candidates = ['app.user.signed-up.v1', 'engineering.deploy.completed.v1'];
    expect(fuzzyMatch('app.user', candidates, { topN: 3 })[0]).toBe('app.user.signed-up.v1');
  });

  it('returns at most topN suggestions', () => {
    const candidates = [
      'app.user.signed-up.v1',
      'app.user.signed-in.v1',
      'app.user.deactivated.v1',
      'app.user.password-changed.v1',
    ];
    const matches = fuzzyMatch('app.user.x', candidates, { topN: 2 });
    expect(matches.length).toBeLessThanOrEqual(2);
  });

  it('returns an empty array when the candidate list is empty', () => {
    expect(fuzzyMatch('anything', [], { topN: 3 })).toEqual([]);
  });

  it('returns the closest candidate first', () => {
    const candidates = ['totally.different.v1', 'app.user.signed-out.v1', 'engineering.x.v1'];
    const matches = fuzzyMatch('app.user.signed-up.v1', candidates, { topN: 3 });
    expect(matches[0]).toBe('app.user.signed-out.v1');
  });

  it('returns the exact match first when the input matches a candidate', () => {
    const candidates = ['app.user.signed-up.v1', 'app.user.signed-in.v1'];
    const matches = fuzzyMatch('app.user.signed-up.v1', candidates, { topN: 3 });
    expect(matches[0]).toBe('app.user.signed-up.v1');
  });

  it('drops candidates that are too distant (relative to the input length)', () => {
    // "abc" vs "totally.unrelated.event.v1" — overwhelmingly different.
    // Implementation may filter; we assert the totally-unrelated candidate
    // does NOT come first.
    const candidates = ['totally.unrelated.event.v1', 'abd'];
    const matches = fuzzyMatch('abc', candidates, { topN: 3 });
    expect(matches[0]).toBe('abd');
  });
});
