/** The mask a reveal adds to the filter's: node `i` shows while `i < shown`, in ingest order. */

/**
 * `hidden` with every node from `shown` on hidden too. Null when nothing is hidden, which
 * is what the painter takes as "draw all"; the filter's own mask is never written to.
 */
export function withReveal(hidden: Uint8Array | null, shown: number | null, total: number): Uint8Array | null {
  if (shown === null || shown >= total) return hidden;
  const mask = hidden === null ? new Uint8Array(total) : hidden.slice();
  for (let i = Math.max(0, shown); i < total; i += 1) mask[i] = 1;
  return mask;
}
