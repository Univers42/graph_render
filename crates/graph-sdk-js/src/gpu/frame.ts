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
 * `2^(r/4)` for `r = 0..3`, each correctly rounded — the four quarter roots of 2.
 *
 * Caveat: V8's `**` is not correctly rounded in general (`2 ** 3.5` is 1 ulp below libm's
 * `exp2(3.5)`), so the table is spelled out rather than computed. `fit` scales these by the
 * exact power of two `2^q`, and rounding commutes with exact power-of-two scaling, so
 * `2^q * QUARTER[r]` is the correctly rounded `2^(step/4)` for every rung.
 */
const QUARTER = [1, 1.189207115002721, 1.4142135623730951, 1.6817928305074292];

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
  const q = Math.floor(step / 4);
  const quarter = QUARTER[step - 4 * q] ?? 1;
  const h = (2 ** q) * quarter;
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
