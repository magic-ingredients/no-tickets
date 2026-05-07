/** Levenshtein edit distance between two strings (iterative, O(n*m)). */
function editDistance(a: string, b: string): number {
  if (a.length === 0) return b.length;
  if (b.length === 0) return a.length;

  let previous: number[] = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i++) {
    const current = [i];
    for (let j = 1; j <= b.length; j++) {
      const cost = a.charCodeAt(i - 1) === b.charCodeAt(j - 1) ? 0 : 1;
      current.push(
        Math.min(
          (previous[j] ?? 0) + 1,
          (current[j - 1] ?? 0) + 1,
          (previous[j - 1] ?? 0) + cost,
        ),
      );
    }
    previous = current;
  }
  return previous[b.length] ?? 0;
}

export interface FuzzyMatchOptions {
  readonly topN: number;
}

/** Return the top-N closest candidates to `input` by Levenshtein distance.
 *  Exact matches win automatically (distance 0); ties broken by candidate
 *  order so the result is stable for repeat invocations. */
export function fuzzyMatch(
  input: string,
  candidates: readonly string[],
  options: FuzzyMatchOptions,
): readonly string[] {
  if (candidates.length === 0) return [];
  const scored = candidates.map((candidate, index) => ({
    candidate,
    distance: editDistance(input, candidate),
    index,
  }));
  scored.sort((a, b) => a.distance - b.distance || a.index - b.index);
  return scored.slice(0, options.topN).map((s) => s.candidate);
}
