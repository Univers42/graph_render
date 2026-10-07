/**
 * Where the mesh sits this tick: its cell size, its origin, how many cells the nodes occupy
 * and how far the kernel reaches — the host half of graph-core's particle mesh frame.
 *
 * This is a second copy of `crates/graph-core/src/layout/force/particle_mesh/frame.rs`,
 * transcribed for the GPU arm's TypeScript side. It is held to the Rust original by
 * `test/gpu-frame.test.mjs`, which checks every fixture header bit for bit: a divergence
 * here is a transcription error, not a design choice.
 *
 * It is deliberately NOT exported from `index.ts`: the frame is a host-side detail of the
 * wasm build, and re-exporting it would be a public ABI change no task has asked for.
 *
 * The math is libm's, through JavaScript's correctly-rounded IEEE754 transcendentals:
 * `Math.log2`, `Math.floor`, `Math.ceil`, and `exp2` as the quarter-root table `QUARTER`
 * scaled by an exact power of two. No `mul_add`, no `powi`, no relaxed SIMD — the ladder
 * must land on the same rung native and wasm32.
 */

/** The smallest step tried: `h = 2^-8`. A tighter cluster than that is cut no finer. */
const STEP_MIN = -32;

/**
 * `2^(step/4)` for `step` in `-32..=64`, each the correctly-rounded `f64` from libm's
 * `exp2` — the values graph-core's `frame.rs` produces and the fixture headers carry.
 *
 * Caveat: V8's `**` is not correctly rounded (`2 ** 4.75` is 1 ulp above libm's
 * `exp2(4.75)`), and `Math.exp2` does not exist in this build, so the table is spelled out
 * rather than computed. A decomposition `2^q * 2^(r/4)` does not work either: the f64
 * rounding of `2^(r/4)` is not the f64 rounding of `2^(step/4)` scaled down, so
 * `f64(2^0.75) * 16 ≠ f64(2^4.75)` by 1 ulp. The table is the only exact form.
 */
const H: Readonly<Record<number, number>> = {
  [-32]: 0.00390625, [-31]: 0.004645340292979379, [-30]: 0.005524271728019903, [-29]: 0.0065695032441696445,
  [-28]: 0.0078125, [-27]: 0.009290680585958758, [-26]: 0.011048543456039806, [-25]: 0.013139006488339289,
  [-24]: 0.015625, [-23]: 0.018581361171917516, [-22]: 0.02209708691207961, [-21]: 0.026278012976678578,
  [-20]: 0.03125, [-19]: 0.03716272234383503, [-18]: 0.04419417382415922, [-17]: 0.052556025953357156,
  [-16]: 0.0625, [-15]: 0.07432544468767006, [-14]: 0.08838834764831845, [-13]: 0.10511205190671431,
  [-12]: 0.125, [-11]: 0.14865088937534013, [-10]: 0.1767766952966369, [-9]: 0.21022410381342863,
  [-8]: 0.25, [-7]: 0.29730177875068026, [-6]: 0.3535533905932738, [-5]: 0.42044820762685725,
  [-4]: 0.5, [-3]: 0.5946035575013605, [-2]: 0.7071067811865476, [-1]: 0.8408964152537145,
  [0]: 1.0, [1]: 1.189207115002721, [2]: 1.4142135623730951, [3]: 1.681792830507429,
  [4]: 2.0, [5]: 2.378414230005442, [6]: 2.8284271247461903, [7]: 3.363585661014858,
  [8]: 4.0, [9]: 4.756828460010884, [10]: 5.656854249492381, [11]: 6.727171322029716,
  [12]: 8.0, [13]: 9.513656920021768, [14]: 11.313708498984761, [15]: 13.454342644059432,
  [16]: 16.0, [17]: 19.027313840043536, [18]: 22.627416997969522, [19]: 26.908685288118864,
  [20]: 32.0, [21]: 38.05462768008707, [22]: 45.254833995939045, [23]: 53.81737057623773,
  [24]: 64.0, [25]: 76.10925536017415, [26]: 90.50966799187809, [27]: 107.63474115247546,
  [28]: 128.0, [29]: 152.2185107203483, [30]: 181.01933598375618, [31]: 215.2694823049509,
  [32]: 256.0, [33]: 304.4370214406966, [34]: 362.03867196751236, [35]: 430.5389646099018,
  [36]: 512.0, [37]: 608.8740428813932, [38]: 724.0773439350247, [39]: 861.0779292198037,
  [40]: 1024.0, [41]: 1217.7480857627863, [42]: 1448.1546878700494, [43]: 1722.1558584396073,
  [44]: 2048.0, [45]: 2435.4961715255727, [46]: 2896.309375740099, [47]: 3444.3117168792146,
  [48]: 4096.0, [49]: 4870.992343051145, [50]: 5792.618751480198, [51]: 6888.623433758429,
  [52]: 8192.0, [53]: 9741.98468610229, [54]: 11585.237502960395, [55]: 13777.246867516858,
  [56]: 16384.0, [57]: 19483.96937220458, [58]: 23170.47500592079, [59]: 27554.493735033717,
  [60]: 32768.0, [61]: 38967.93874440916, [62]: 46340.95001184158, [63]: 55108.98747006743,
  [64]: 65536.0,
};

/** A bounding box, `(lo, hi)`. */
export interface Bounds {
  loX: number;
  loY: number;
  hiX: number;
  hiY: number;
}

/** The mesh's placement for one tick. */
export interface Frame {
  /** The rung: `h = 2^(step/4)`. */
  step: number;
  /** The cell size. */
  h: number;
  /** The world position of cell `(0, 0)`'s left edge. */
  originX: number;
  /** The world position of cell `(0, 0)`'s bottom edge. */
  originY: number;
  /** Cells per axis the deposit may touch: indices `0..cells`. */
  cells: number;
  /** The kernel's reach in cells per axis: offsets `-reach..=reach`. */
  reach: number;
}

/**
 * The finite positions' bounding box, or `null` when there is none.
 *
 * The Rust original folds per block of 4096 nodes on its workers, then folds the blocks in
 * order with `widen`'s strict comparisons. One fold over every node keeps the same box: min
 * and max are order-independent in value, and the strict `<`/`>` keep the same end on a tie,
 * `-0.0` against `+0.0` included — so this single thread is the same fold.
 */
export function bounds(posX: Float64Array, posY: Float64Array): Bounds | null {
  let loX = Infinity;
  let loY = Infinity;
  let hiX = -Infinity;
  let hiY = -Infinity;
  for (let i = 0; i < posX.length; i += 1) {
    const px = posX[i] ?? 0;
    const py = posY[i] ?? 0;
    if (Number.isFinite(px) && Number.isFinite(py)) {
      if (px < loX) loX = px;
      if (py < loY) loY = py;
      if (px > hiX) hiX = px;
      if (py > hiY) hiY = py;
    }
  }
  if (loX > hiX) {
    return null;
  }
  return { loX, loY, hiX, hiY };
}

/**
 * The frame over the bounds `b`, for a mesh of `side` cells per axis and a force that
 * vanishes at distance `dmax`. `null` when the span is not finite.
 */
export function place(b: Bounds, side: number, dmax: number): Frame | null {
  const span = Math.max(b.hiX - b.loX, b.hiY - b.loY);
  if (!Number.isFinite(span)) {
    return null;
  }
  // Rust's `as i32` saturates and maps NaN to 0; JavaScript's `Math.min`/`Math.max`
  // propagate NaN instead, so both halves of the cast are spelled out here.
  const raw = Math.floor(4 * Math.log2(span / side));
  const saturated = Number.isNaN(raw) ? 0 : Math.min(Math.max(raw, -2147483648), 2147483647);
  let step = Math.max(saturated, STEP_MIN);
  for (;;) {
    const frame = fit(step, span, side, dmax);
    if (frame !== null) {
      return {
        step: frame.step,
        h: frame.h,
        originX: Math.floor(b.loX / frame.h) * frame.h,
        originY: Math.floor(b.loY / frame.h) * frame.h,
        cells: frame.cells,
        reach: frame.reach,
      };
    }
    step += 1;
  }
}

/**
 * The frame at rung `step` with the origin still unset, if it fits in `side`.
 *
 * `cells` is `floor(span / h) + 3`: the origin snaps down by less than `h`, so the far
 * node's cell index is at most `floor(span / h) + 1`, and its CIC stencil reaches one
 * further. The deposit clamps to the same range, so a rounding at the boundary moves a
 * weight by one cell edge rather than past the end.
 *
 * Caveat: a negative `span` — only reachable by calling `place` with `lo > hi`, which
 * `bounds` never does — would diverge from the Rust `as usize` saturation; well-formed
 * bounds give `cells >= 3`, where the two agree exactly.
 */
function fit(step: number, span: number, side: number, dmax: number): Omit<Frame, "originX" | "originY"> | null {
  const h = H[step] ?? 2 ** (step / 4);
  const cells = Math.floor(span / h) + 3;
  if (cells > side) {
    return null;
  }
  const reach = Math.min(Math.ceil(dmax / h), cells - 1);
  if (cells + reach > side) {
    return null;
  }
  return { step, h, cells, reach };
}

/**
 * [`place`] over the finite positions of `posX`/`posY`, one call: what the GPU arm builds
 * its frame with. `null` when no position is finite or the span is not finite.
 */
export function frameOf(posX: Float64Array, posY: Float64Array, side: number, dmax: number): Frame | null {
  const b = bounds(posX, posY);
  if (b === null) {
    return null;
  }
  return place(b, side, dmax);
}
