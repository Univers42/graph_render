/**
 * `bounds-collide.ts` — what the collide arm's numbers are held to, and its one exactness
 * check.
 *
 * ## `order`, the check no bound can make
 *
 * The resolve sums each node's candidates in the CPU's order, and that order is the member
 * lists' (`collide.rs:150-165`: a stable counting sort, so each bucket lists its nodes by
 * ascending index). A device that listed them in another order would sum the same terms in
 * another order: a rounding difference, under every `f32` bound (plan `:1333-1338`). So the
 * order is read back and checked as a fact, and `--break collide-order` is what proves the
 * check reads it.
 */

/**
 * `null` when every bucket's member list is strictly ascending and together they hold each of
 * the `n` nodes once; otherwise the first violation, in words, for the failure line.
 *
 * `start` is the scan's output, `buckets + 1` entries, bucket `b`'s span `start[b]..start[b + 1]`.
 */
export function orderCheck(order: Uint32Array, start: Uint32Array, n: number): string | null {
  const buckets = start.length - 1;
  if (start[0] !== 0 || start[buckets] !== n || order.length < n) {
    return `the spans run ${start[0]}..${start[buckets]}, not n = ${n}`;
  }
  const seen = new Uint8Array(n);
  for (let b = 0; b < buckets; b += 1) {
    const lo = start[b] ?? 0;
    const hi = start[b + 1] ?? 0;
    for (let q = lo; q < hi; q += 1) {
      const node = order[q] ?? n;
      if (node >= n || seen[node] === 1) {
        return `slot ${q} holds node ${node}, out of range or listed twice`;
      }
      seen[node] = 1;
      if (q > lo && node <= (order[q - 1] ?? 0)) {
        return `bucket ${b} is not ascending at slot ${q}: node ${order[q - 1]} then ${node}`;
      }
    }
  }
  return null;
}
