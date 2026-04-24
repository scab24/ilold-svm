// Minimal substring + subsequence scorer for the Cmd+K command palette.
// No external dependency; the command set is small (≤100 items typical)
// so even an O(n·m) scan is negligible on the keystroke loop.
//
// Scoring tiers — higher is better, negative means no match:
//   1000 … exact case-insensitive match
//    500 … prefix match (shorter label wins)
//    100 … contiguous substring anywhere
//     10 … non-contiguous character subsequence
//     -1 … no match at all

export function score(label: string, query: string): number {
  if (!query) return 1; // empty query keeps original order via stable sort
  const l = label.toLowerCase();
  const q = query.toLowerCase();
  if (l === q) return 1000;
  if (l.startsWith(q)) return 500 - l.length;
  const idx = l.indexOf(q);
  if (idx >= 0) return 100 - idx - Math.floor(l.length / 4);
  // Char subsequence fallback — matches "dpst" against "deposit"
  let qi = 0;
  for (let i = 0; i < l.length && qi < q.length; i++) {
    if (l[i] === q[qi]) qi++;
  }
  if (qi === q.length) return 10 - Math.floor(l.length / 4);
  return -1;
}

// Score a label against a query AND a list of extra keywords. Returns the
// best score found — lets `{ label: "Center canvas", keywords: ["fit", "zoom"] }`
// also match "fit".
export function scoreWithKeywords(
  label: string,
  keywords: readonly string[] | undefined,
  query: string,
): number {
  let best = score(label, query);
  if (keywords) {
    for (const k of keywords) {
      const s = score(k, query);
      if (s > best) best = s;
    }
  }
  return best;
}
