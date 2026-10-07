/**
 * `collide-host.ts` — the collide pass's host halves: the grid walk over a fixture's positions,
 * the CPU's `resolve` transcribed in `f64`, and the `f32`-narrowing floor the guard adds
 * (Amendment 3, `docs/decisions/gpu-g1.md`).
 *
 * The device narrows every position to `f32` before it hashes or resolves (`gpu/collide.ts`,
 * `Math.fround`). That rounding is `2⁻²⁴·|x|` per coordinate, so it scales with the coordinate
 * extent and not with the collide radius — the seed spiral reaches `12·√(i+1)`, an extent of
 * 12 000 at 1M against 377 at 1k. Amendment 1 bounded only the arithmetic, so its `P = reach / 2`
 * scale misses the input. `collideFloor` measures the input's error on the host, in `f64`, with
 * the grid the device sorts with — the same instrument `k_c` already is.
 *
 * ## The walk
 *
 * One walk visits every node's contacts: the counting sort into the hashed cell list, then a
 * gather over each node's nine neighbour cells. `maxContacts` counts a node's contacts through
 * it and `hostCollide` sums a node's pushes through it, so no walk is written twice.
 *
 * Caveat: `hostCollide` does not port the CPU's coincidence jiggle (`collide.rs:245-252`), which
 * nudges an exactly-zero axis by at most `5e-7` (`rng.rs:47`, `(random() - 0.5) * 1e-6`). An
 * exactly zero axis contributes its zero component instead, so a pair coincident on one axis
 * resolves as the jiggle's limit. A pair coincident on both axes would divide by zero, and no
 * committed fixture has one — `the_host_collide_reproduces_the_fixture` holds on both 1k
 * fixtures, and the numpy analysis (`harness/gpu-collide-floor.py`) reads the same.
 */

import { components } from "./bounds.ts";
import { cellOf } from "./collide.ts";
import type { Grid } from "./collide.ts";

/** The bucket of cell `(cx, cy)`: `hash.rs:24-25` with the row table. */
function bucketOf(cx: number, cy: number, grid: Grid): number {
  const at = Math.min(Math.max(cy + 1, 0), grid.rows.length - 1);
  return ((grid.rows[at] ?? 0) + cx) & grid.mask;
}

/** The buckets a query from cell `(cx, cy)` reads, each once, in `Grid::reads` order. */
function readsBuckets(cx: number, cy: number, grid: Grid): number[] {
  const at: number[] = [];
  const firsts: number[] = [];
  for (let row = 0; row < 3; row += 1) {
    const first = bucketOf(cx - 1, cy - 1 + row, grid);
    for (let t = 0; t < 3; t += 1) {
      const b = (first + t) & grid.mask;
      let seen = false;
      for (let e = 0; e < row; e += 1) seen = seen || (((b - (firsts[e] ?? 0)) & grid.mask) < 3);
      if (!seen) at.push(b);
    }
    firsts[row] = first;
  }
  return at;
}

/**
 * The grid walk: sort every node into the hashed cell list, then visit each node's contacts —
 * the candidates in its nine neighbour cells that pass the CPU's `d2` test (`collide.rs:242`).
 *
 * `visit(i, j)` is called once per ordered contact, in ascending `i` then the bucket order the
 * device reads, so a caller summing pushes sums them in the CPU's order. The cell of every node
 * comes from the positions it is given, so the caller narrows them or not.
 *
 * Caveat: the walk is the device's candidate set, not the CPU's exact pair set — a pair whose
 * `f32` distance rounds across the diameter is visited by one arm and not the other, and its
 * push is near zero, so the floor moves by less than the arithmetic term (Amendment 3).
 */
function walkContacts(
  posX: Float64Array,
  posY: Float64Array,
  grid: Grid,
  visit: (i: number, j: number) => void,
): void {
  const n = posX.length;
  const bucket = new Uint32Array(n);
  for (let i = 0; i < n; i += 1) {
    bucket[i] = bucketOf(cellOf(posX[i] ?? 0, grid.originX, grid.invSize), cellOf(posY[i] ?? 0, grid.originY, grid.invSize), grid);
  }
  const counts = new Uint32Array(grid.buckets);
  for (let i = 0; i < n; i += 1) counts[bucket[i] ?? 0] = (counts[bucket[i] ?? 0] ?? 0) + 1;
  const start = new Uint32Array(grid.buckets + 1);
  for (let b = 0; b < grid.buckets; b += 1) start[b + 1] = (start[b] ?? 0) + (counts[b] ?? 0);
  const order = new Uint32Array(n);
  const cursor = start.slice(0, grid.buckets);
  for (let i = 0; i < n; i += 1) {
    const b = bucket[i] ?? 0;
    order[cursor[b] ?? 0] = i;
    cursor[b] = (cursor[b] ?? 0) + 1;
  }
  for (let i = 0; i < n; i += 1) {
    const cx = cellOf(posX[i] ?? 0, grid.originX, grid.invSize);
    const cy = cellOf(posY[i] ?? 0, grid.originY, grid.invSize);
    for (const b of readsBuckets(cx, cy, grid)) {
      for (let q = start[b] ?? 0; q < (start[b + 1] ?? 0); q += 1) {
        const j = order[q] ?? 0;
        if (j === i) continue;
        const dx = (posX[i] ?? 0) - (posX[j] ?? 0);
        const dy = (posY[i] ?? 0) - (posY[j] ?? 0);
        const l = dx * dx + dy * dy;
        if (Number.isNaN(l) || l >= grid.d2) continue;
        visit(i, j);
      }
    }
  }
}

/**
 * The largest number of contacts of any one node, in `f64`, over the fixture's positions.
 *
 * A contact is a pair closer than `reach` (the CPU's `d2` test, `collide.rs:242`), counted on the
 * host in `f64` from the fixture's own positions. The neighbourhood is the grid's — the same
 * `cellOf` and row table the device sorts with — so the count is the device's candidate count,
 * with the distance test the CPU's.
 *
 * Caveat: the count is the fixture's own crowd. A denser overlap raises it, so the guard is only
 * as good as the fixture's measured crowd, the same limit condition 8 states for link.
 */
export function maxContacts(posX: Float64Array, posY: Float64Array, grid: Grid): number {
  const n = posX.length;
  const crowds = new Uint32Array(n);
  walkContacts(posX, posY, grid, (i) => {
    crowds[i] = (crowds[i] ?? 0) + 1;
  });
  let max = 0;
  for (let i = 0; i < n; i += 1) {
    const c = crowds[i] ?? 0;
    if (c > max) max = c;
  }
  return max;
}

/**
 * The CPU's `resolve` (`collide.rs:235-257`) transcribed in `f64`, per node, over the node's
 * contacts as the walk finds them.
 *
 * `l = dx² + dy²`; skip when `l` is NaN or `l ≥ d2`; `dist = √l`;
 * `push = (reach − dist) / dist · 0.5`; add `dx·push` and `dy·push`. The cell of every node comes
 * from the positions it is given, so the caller narrows them or not.
 *
 * Caveat: the coincidence jiggle (`collide.rs:245-252`) is not ported. It nudges an exactly-zero
 * axis by at most `5e-7` (`rng.rs:47`), so an exactly zero axis contributes its zero component
 * here — a one-axis-coincident pair resolves as the jiggle's limit, and a both-axis-coincident
 * pair would divide by zero. No committed fixture has one: `the_host_collide_reproduces_the_fixture`
 * holds on both 1k fixtures, and the numpy analysis reads the same.
 */
export function hostCollide(posX: Float64Array, posY: Float64Array, grid: Grid): { x: Float64Array; y: Float64Array } {
  const n = posX.length;
  const x = new Float64Array(n);
  const y = new Float64Array(n);
  walkContacts(posX, posY, grid, (i, j) => {
    const dx = (posX[i] ?? 0) - (posX[j] ?? 0);
    const dy = (posY[i] ?? 0) - (posY[j] ?? 0);
    const dist = Math.sqrt(dx * dx + dy * dy);
    const push = ((grid.reach - dist) / dist) * 0.5;
    x[i] = (x[i] ?? 0) + dx * push;
    y[i] = (y[i] ?? 0) + dy * push;
  });
  return { x, y };
}

/**
 * The guard's input floor (Amendment 3): the absolute rms, over the `2n` components in
 * `components()`'s order, of `hostCollide` on the positions narrowed with `Math.fround` against
 * `hostCollide` on the `f64` positions.
 *
 * The device narrows every position to `f32` before it hashes or resolves, so no `f32`
 * implementation can sit under this floor. It is the triangle inequality's first term: the arm's
 * distance from the narrowed reference is Amendment 1's arithmetic term, and the narrowed
 * reference's distance from the fixture is this floor.
 *
 * Caveat: the floor is the fixture's own, from one start, as `k_c` is. Another seed's floor is
 * recomputed, never carried over. The pair set at a contact's exact boundary can differ between
 * the narrowed and the `f64` positions; the push there is ~0, so it moves the floor by less than
 * the arithmetic term.
 */
export function collideFloor(posX: Float64Array, posY: Float64Array, grid: Grid): number {
  const exact = hostCollide(posX, posY, grid);
  const narrowed = hostCollide(narrow(posX), narrow(posY), grid);
  const diff = components(sub(exact.x, narrowed.x), sub(exact.y, narrowed.y));
  return rms(diff);
}

/** The positions narrowed once, the way the device uploads them (`collide.ts:231-232`). */
function narrow(pos: Float64Array): Float64Array {
  const out = new Float64Array(pos.length);
  for (let k = 0; k < pos.length; k += 1) out[k] = Math.fround(pos[k] ?? 0);
  return out;
}

/** `a − b` componentwise. */
function sub(a: Float64Array, b: Float64Array): Float64Array {
  const out = new Float64Array(a.length);
  for (let k = 0; k < a.length; k += 1) out[k] = (a[k] ?? 0) - (b[k] ?? 0);
  return out;
}

/** The rms over every component, in `components()`'s order. */
function rms(values: Float64Array): number {
  let sum = 0;
  for (let k = 0; k < values.length; k += 1) {
    const v = values[k] ?? 0;
    sum += v * v;
  }
  return Math.sqrt(sum / values.length);
}
