/**
 * The edge sample of a moving frame. A zoomed-out 1M-node graph covers the screen about 480
 * times over with its 2M chords (target/perf-p5d-before-1m-r1: 49.6 ms of GPU edge time per
 * moving frame), and no pixel can show more than the first few dozen crossings, so a moving
 * frame draws a spread sample of the edges instead of all of them and scales the alpha up by
 * what it skipped (compensate), the way stacked strands read the same. The sample never touches
 * a settled picture: the fill still draws every edge (still.ts), so a parity-sized graph (about
 * 1.3 coverings) and every settled frame are the full draw.
 *
 * The step is measured once per placement change from the drawn pairs (sync.ts) and applied
 * per frame from the camera scale (draw.ts).
 */

/** What one placement measured of its edges: mean chord length and bounding span, in world units. */
export interface EdgeShape {
  readonly mean: number;
  readonly spanX: number;
  readonly spanY: number;
}

/** The screen the step is judged against: the camera scale and the canvas, in CSS pixels. */
export interface ScreenTarget {
  readonly scale: number;
  readonly width: number;
  readonly height: number;
  readonly dpr: number;
}

/** Pairs one `measurePairs` pass samples; a stride over the spread order still hits every corner. */
const MEASURED = 65536;
/** Coverings a frame leaves at the floor: about three dozen crossings survive a step of eight. */
const COVER = 24;
/** The largest step: one edge in eight. */
const CEILING = 8;
/** The smallest share of the graph's area the viewport may take; below this the estimate lies. */
const FILLS = 0.125;

/**
 * Mean chord length and bounding span over `pairs` of `index`, sampling at most MEASURED of
 * them (the pairs are in spread order, so a stride samples the whole graph rather than one
 * corner of it). Zero pairs measure a zero shape, which draws everything.
 */
export function measurePairs(x: Float32Array, y: Float32Array, index: Uint32Array): EdgeShape {
  const pairs = Math.floor(index.length / 2);
  const stride = Math.max(1, Math.floor(pairs / MEASURED));
  let sum = 0;
  let taken = 0;
  let lowX = Infinity;
  let lowY = Infinity;
  let highX = -Infinity;
  let highY = -Infinity;
  for (let edge = 0; edge < pairs; edge += stride) {
    const from = index[2 * edge] ?? 0;
    const to = index[2 * edge + 1] ?? 0;
    const ax = x[from] ?? 0;
    const ay = y[from] ?? 0;
    const bx = x[to] ?? 0;
    const by = y[to] ?? 0;
    sum += Math.sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by));
    if (ax < lowX) lowX = ax;
    if (bx < lowX) lowX = bx;
    if (ax > highX) highX = ax;
    if (bx > highX) highX = bx;
    if (ay < lowY) lowY = ay;
    if (by < lowY) lowY = by;
    if (ay > highY) highY = ay;
    if (by > highY) highY = by;
    taken += 1;
  }
  if (taken === 0) return { mean: 0, spanX: 0, spanY: 0 };
  return { mean: sum / taken, spanX: highX - lowX, spanY: highY - lowY };
}

/**
 * How many times over `drawn` edges of this shape cover the target's area, drawn as one edge
 * in `step`: the coverings divided by COVER, clamped to [1, CEILING], and 1 whenever the shape
 * is missing or the target is not filled.
 * Ponytail: the estimate counts every chord in full even where the viewport clips it, so a
 * graph up to eight times the screen's area (FILLS) reads denser than it is and the step
 * over-thins (wrong direction: a sparse region lighter than the full draw, never darker);
 * a rare hub-spanning chord inflates the mean the same way. Failing input: a graph whose
 * edges are almost all short with a few screen-spanning ones, just inside FILLS. Escape
 * hatch: raise FILLS toward 1 or COVER above 24; zooming in past FILLS already disables it.
 */
export function sampleStep(shape: EdgeShape | null, drawn: number, target: ScreenTarget): number {
  if (shape === null || drawn < 1) return 1;
  const graph = shape.spanX * target.scale * shape.spanY * target.scale;
  const screen = target.width * target.height;
  if (!(screen / graph >= FILLS)) return 1;
  const coverings = (drawn * shape.mean * target.scale) / (screen * target.dpr);
  const step = Math.floor(coverings / COVER);
  if (!(step > 1)) return 1;
  return Math.min(step, CEILING);
}

/**
 * What `u_alpha` is multiplied by so `step` sampled edges read as the coverage of `step`
 * stacked lines of `alpha`: (1 - (1 - alpha)^step) / alpha, which is 1 at a step of 1 and
 * never pushes the flat edge colour past opaque. Gradient edges carry their own alpha and
 * only get this multiplier, which is exact where it equals the theme's.
 */
export function compensate(alpha: number, step: number): number {
  if (step <= 1 || alpha <= 0 || alpha >= 1) return 1;
  return (1 - Math.pow(1 - alpha, step)) / alpha;
}
