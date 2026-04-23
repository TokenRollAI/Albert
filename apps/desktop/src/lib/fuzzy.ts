/**
 * Tiny fuzzy matcher tuned for the command palette. Returns a positive
 * score (higher = better) when every character of `query` appears in
 * `text` in order, or `null` when it doesn't match at all. Contiguous
 * matches and matches at word boundaries score higher, so typing `gup`
 * against `GET /users/{id}` surfaces before `GET /products/{upc}`.
 *
 * Deliberately dependency-free and stable for a small candidate set
 * (< 500 entries is our realistic ceiling). Not intended to compete with
 * fzf; just good enough that users rarely have to refine their typing.
 */
export interface FuzzyMatch {
  score: number;
  /** Zero-based indexes into `text` of each matched character. */
  indexes: number[];
}

export function fuzzyMatch(
  query: string,
  text: string
): FuzzyMatch | null {
  const q = query.trim().toLowerCase();
  if (!q) return { score: 0, indexes: [] };
  const lowerText = text.toLowerCase();
  const indexes: number[] = [];

  let score = 0;
  let lastIndex = -1;
  let consecutive = 0;

  for (const ch of q) {
    let found = -1;
    // Resume search just after the previous match so characters match in
    // order — the invariant that makes this a fuzzy match, not a bag match.
    for (let i = lastIndex + 1; i < lowerText.length; i++) {
      if (lowerText[i] === ch) {
        found = i;
        break;
      }
    }
    if (found === -1) return null;

    // Word-boundary bonus: first char, or the char before is a separator.
    const prev = found === 0 ? " " : lowerText[found - 1];
    const atBoundary = /[\s/\-_:.\[\]]/.test(prev) || found === 0;
    if (atBoundary) score += 8;
    else score += 1;

    if (found === lastIndex + 1) {
      consecutive += 1;
      // 3× multiplier so contiguous runs decisively beat gapped matches
      // even when the gap crosses a word boundary (which also carries a
      // bonus). Capped at 10 so a pathologically long shared prefix can't
      // swamp every other signal.
      score += Math.min(consecutive, 10) * 3;
    } else {
      consecutive = 0;
    }

    indexes.push(found);
    lastIndex = found;
  }

  return { score, indexes };
}

/**
 * Filter + sort a list of candidates by fuzzy score. Non-matching items
 * are dropped. Stable ordering: ties preserve input order, which keeps
 * keyboard navigation predictable.
 */
export function fuzzyFilter<T>(
  query: string,
  candidates: T[],
  getLabel: (candidate: T) => string
): Array<{ item: T; match: FuzzyMatch }> {
  const q = query.trim();
  const scored: Array<{ item: T; match: FuzzyMatch; index: number }> = [];
  candidates.forEach((item, index) => {
    const match = fuzzyMatch(q, getLabel(item));
    if (match !== null) {
      scored.push({ item, match, index });
    }
  });
  scored.sort((a, b) => {
    if (b.match.score !== a.match.score) {
      return b.match.score - a.match.score;
    }
    return a.index - b.index;
  });
  return scored.map(({ item, match }) => ({ item, match }));
}
