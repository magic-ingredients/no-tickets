import { describe, it, expect } from 'vitest';
import { fuzzyMatch } from './fuzzy-match.js';

describe('fuzzyMatch', () => {
  it('ranks the closest candidate first by edit distance (a short input matches the long candidate that shares its prefix)', () => {
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

  it('returns the exact match first even when it is NOT first in the input list', () => {
    // Place exact match at index 1 so a regression that returns input order
    // can't pass.
    const candidates = ['app.user.signed-in.v1', 'app.user.signed-up.v1'];
    const matches = fuzzyMatch('app.user.signed-up.v1', candidates, { topN: 3 });
    expect(matches[0]).toBe('app.user.signed-up.v1');
  });

  it('ranks the closer candidate ahead of a far-away one', () => {
    const candidates = ['totally.unrelated.event.v1', 'abd'];
    const matches = fuzzyMatch('abc', candidates, { topN: 3 });
    expect(matches[0]).toBe('abd');
    expect(matches[1]).toBe('totally.unrelated.event.v1');
  });

  it('breaks ties by input order (stable sort)', () => {
    // Two candidates with identical distance from "ab".
    const matches = fuzzyMatch('ab', ['ax', 'ay'], { topN: 2 });
    expect(matches).toEqual(['ax', 'ay']);
  });
});
