/**
 * `collide.ts` — the collide pass on a WebGPU device: the grid the host derives, the
 * dispatches, the read-back.
 *
 * The CPU pass is `collide.rs`: a counting sort of the nodes into a hashed cell list one
 * diameter wide, then a gather over each node's nine neighbour cells. The device runs the
 * same shape in four stages, `collide_hash`, `collide_scan`, `collide_scatter` and
 * `collide_resolve`, one module each under `kernels/`.
 *
 * ## What the host derives, and why it is the host's
 *
 * The fixture carries positions and no collide parameters, so the grid comes from the frozen
 * set the emitter used (`ForceParams::default`, `gpu_fixtures/settle.rs:87-92`): radius 16, so
 * cells 32 wide. The origin is the finite positions' minimum, as `frame::bounds` folds it
 * over the at-rest projection `x + 0` (`collide.rs:146`); `f32` rounding is monotonic, so the
 * narrowed minimum is the minimum of the narrowed positions. The row table is `hash.rs:30`'s
 * 64-bit product per row, evaluated in `BigInt` (`kernels/collide.wgsl.ts` says why).
 */

/** The collide radius at the frozen parameters (`params.rs:71`); the cell is its diameter. */
const COLLIDE_RADIUS = 16;

/** `hash.rs:30`'s multiplier. */
const ROW_HASH = 0x9e3779b97f4a7c15n;

/** The grid every collide stage reads: the uniform's numbers and the row table. */
export interface Grid {
  /** The finite positions' minimum, narrowed to `f32`. */
  readonly originX: number;
  readonly originY: number;
  /** `1 / diameter`, narrowed to `f32`. */
  readonly invSize: number;
  /** The diameter and its square (`collide.rs:262-273`). */
  readonly reach: number;
  readonly d2: number;
  readonly buckets: number;
  readonly mask: number;
  /** `rows[cy + 1]` is row `cy`'s bucket for cell 0, for `cy` in `-1..=rowMax + 1`. */
  readonly rows: Uint32Array;
}

/** The scan's workgroups: `blocks` for its first and third dispatch, `chunk` sums each in its second. */
export function scanPlan(buckets: number): { readonly blocks: number; readonly chunk: number } {
  const blocks = Math.ceil(buckets / 256);
  return { blocks, chunk: Math.ceil(blocks / 256) };
}

/** The grid over one fixture's positions. */
export function gridFor(posX: Float64Array, posY: Float64Array): Grid {
  const n = posX.length;
  const diameter = 2 * COLLIDE_RADIUS;
  const d2 = diameter * diameter;
  const reach = Math.sqrt(d2);
  const [lowX, lowY] = finiteMinimum(posX, posY);
  const originX = Math.fround(lowX);
  const originY = Math.fround(lowY);
  const invSize = Math.fround(1 / reach);
  // collide.rs:68: (2n).next_power_of_two().max(4).
  const buckets = Math.max(4, 2 ** (32 - Math.clz32(Math.max(2, 2 * n) - 1)));
  let rowMax = 0;
  for (let k = 0; k < n; k += 1) {
    const y = posY[k] ?? 0;
    if (Number.isFinite(y) && Number.isFinite(posX[k] ?? 0)) {
      rowMax = Math.max(rowMax, cellOf(Math.fround(y), originY, invSize));
    }
  }
  const rows = rowTable(rowMax, 64 - Math.log2(buckets));
  return { originX, originY, invSize, reach, d2, buckets, mask: buckets - 1, rows };
}

/**
 * The device's `axis_cell`, op for op: `i32((v - o) * inv)` on `f32` values, each operation
 * correctly rounded as WGSL's `-` and `*` are, truncated and saturated as `i32(f32)` is.
 *
 * Caveat: it agrees with the CPU's `f64` cell except at a cell edge, where the narrowed
 * position can round across it; `the_collide_hash_matches_the_cpu_s_cells` holds on every node
 * of both 1k fixtures, and a crossing elsewhere costs only the order of one sum.
 */
export function cellOf(v: number, o: number, inv: number): number {
  const u = Math.fround(Math.fround(v - o) * inv);
  if (Number.isNaN(u)) {
    return 0;
  }
  return Math.trunc(Math.min(Math.max(u, -(2 ** 31)), 2 ** 31 - 1));
}

/** `(lo x, lo y)` over the nodes whose two coordinates are finite; `(0, 0)` when none is. */
function finiteMinimum(posX: Float64Array, posY: Float64Array): [number, number] {
  let lowX = Number.POSITIVE_INFINITY;
  let lowY = Number.POSITIVE_INFINITY;
  for (let k = 0; k < posX.length; k += 1) {
    const x = posX[k] ?? 0;
    const y = posY[k] ?? 0;
    if (Number.isFinite(x) && Number.isFinite(y)) {
      lowX = x < lowX ? x : lowX;
      lowY = y < lowY ? y : lowY;
    }
  }
  return lowX === Number.POSITIVE_INFINITY ? [0, 0] : [lowX, lowY];
}

/** Rows `-1..=rowMax + 1`, each `hash.rs:30`'s `(cy as u64).wrapping_mul(K) >> shift`. */
function rowTable(rowMax: number, shift: number): Uint32Array {
  const table = new Uint32Array(rowMax + 3);
  for (let at = 0; at < table.length; at += 1) {
    const cy = BigInt.asUintN(64, BigInt(at - 1));
    table[at] = Number(BigInt.asUintN(64, cy * ROW_HASH) >> BigInt(shift));
  }
  return table;
}
