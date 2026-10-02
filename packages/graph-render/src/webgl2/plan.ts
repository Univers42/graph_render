/**
 * The pure half of the GPU layer: which backend draws a frame, and the arrays it uploads.
 * Nothing here touches a GL context, so all of it runs under `node --test`.
 */

/** `auto` takes the GPU layer for a large scene when the browser has one. */
export type BackendChoice = "auto" | "canvas2d" | "webgl2";

export const BACKEND_CHOICES: readonly BackendChoice[] = ["auto", "canvas2d", "webgl2"];

/**
 * Nodes plus edges from which `auto` hands the bulk of a frame to the GPU.
 * Caveat: one number for every device. Under software raster the 2D painter already drew
 * 2000 nodes at 1.5 fps (docs/measurements/studio-perf-baseline.md), so the threshold is
 * where the GPU layer's own fixed cost (one bitmap blit per frame) stops mattering, not a
 * measured crossover on a real GPU; a host that knows better passes `webgl2` or `canvas2d`.
 */
export const BULK_THRESHOLD = 8192;

export function backendOf(text: string | null | undefined): BackendChoice {
  return BACKEND_CHOICES.find((choice) => choice === text) ?? "auto";
}

/** True when the GPU layer draws the bulk of this scene. */
export function bulkWanted(choice: BackendChoice, elements: number, available: boolean): boolean {
  if (!available || choice === "canvas2d") return false;
  return choice === "webgl2" || elements >= BULK_THRESHOLD;
}

/**
 * The element list of the edge pass: two node indices per drawn edge. An edge touching a
 * hidden node is left out, as `screenEnds` leaves it out of the 2D pass.
 */
export function visibleEdges(source: Uint32Array, target: Uint32Array, hidden: Uint8Array | null): Uint32Array {
  const count = Math.min(source.length, target.length);
  const out = new Uint32Array(count * 2);
  let at = 0;
  for (let edge = 0; edge < count; edge += 1) {
    const s = source[edge] ?? 0;
    const t = target[edge] ?? 0;
    if (hidden !== null && (hidden[s] === 1 || hidden[t] === 1)) continue;
    out[at] = s;
    out[at + 1] = t;
    at += 2;
  }
  return at === out.length ? out : out.slice(0, at);
}

/**
 * Half the width and height of every node in world units, the shape the node pass draws:
 * a disc's radius on both axes, or a box's half sides (`w` and `h` are null unless the
 * frame's nodes are boxes). A hidden node gets -1, which the vertex shader turns into nothing.
 */
export function nodeHalves(input: { extent: Float32Array; w: Float32Array | null; h: Float32Array | null; hidden: Uint8Array | null }): Float32Array {
  const count = input.extent.length;
  const out = new Float32Array(count * 2);
  const { w, h, hidden } = input;
  for (let node = 0; node < count; node += 1) {
    const gone = hidden?.[node] === 1;
    const radius = input.extent[node] ?? 0;
    out[2 * node] = gone ? -1 : w === null ? radius : (w[node] ?? 0) / 2;
    out[2 * node + 1] = gone ? -1 : h === null ? radius : (h[node] ?? 0) / 2;
  }
  return out;
}

/** The largest half side `nodeHalves` wrote, in world units; 0 when every node is hidden. */
export function largestHalf(halves: Float32Array): number {
  let largest = 0;
  for (const half of halves) if (half > largest) largest = half;
  return largest;
}

/** Positions 0..count-1 in bit-reversed order, so that every prefix is spread over the whole range. */
export function spreadOrder(count: number): Uint32Array {
  let size = 1;
  while (size < count) size *= 2;
  const reversed = new Uint32Array(size);
  for (let length = 1; length < size; length *= 2) {
    for (let at = 0; at < length; at += 1) {
      const doubled = (reversed[at] ?? 0) * 2;
      reversed[at] = doubled;
      reversed[at + length] = doubled + 1;
    }
  }
  return size === count ? reversed : reversed.filter((position) => position < count);
}

/**
 * The pairs of `visibleEdges` in `spreadOrder`: a moving frame draws a prefix of them, which
 * samples the whole graph the way the 2D painter's stride does rather than one corner of it.
 */
export function spreadPairs(pairs: Uint32Array): Uint32Array {
  const order = spreadOrder(Math.floor(pairs.length / 2));
  const out = new Uint32Array(order.length * 2);
  for (let at = 0; at < order.length; at += 1) {
    const edge = order[at] ?? 0;
    out[2 * at] = pairs[2 * edge] ?? 0;
    out[2 * at + 1] = pairs[2 * edge + 1] ?? 0;
  }
  return out;
}

/** The shown nodes of `nodeHalves`' output in spread order: a moving frame draws a prefix of them. */
export function spreadShown(halves: Float32Array): Uint32Array {
  return spreadOrder(Math.floor(halves.length / 2)).filter((node) => (halves[2 * node] ?? -1) >= 0);
}

/** A moving GPU frame above this many milliseconds halves its budget, below FAST_MS doubles it. */
export const SLOW_MS = 24;
export const FAST_MS = 12;
/**
 * The fewest edges, and nodes, a moving GPU frame draws. It starts at MOVING_BUDGET and may fall
 * this far: on software raster a random 1M-node layout spends its frame rasterising MOVING_BUDGET
 * screen-long lines (`transferToImageBitmap` was 84% of a zoom's CPU profile), at 17-19 fps.
 */
export const MOVING_FLOOR = 2048;

/**
 * The edges, and the nodes, the next moving frame draws, from what the last one cost: halved
 * above SLOW_MS, doubled below FAST_MS, kept between, never below MOVING_FLOOR nor above the
 * whole set.
 * Caveat: `ms` is the time the CPU waited on the layer. A driver that returns before the GPU
 * is done (most hardware GPUs) reports less than the frame costs, so the budget climbs to the
 * whole set and a GPU-bound frame is not paced; software raster waits, and is.
 */
export function nextBudget(budget: number, ms: number, total: number): number {
  const next = ms > SLOW_MS ? budget / 2 : ms < FAST_MS ? budget * 2 : budget;
  return Math.max(Math.min(MOVING_FLOOR, total), Math.min(total, Math.floor(next)));
}

/** What `onScreen` culls against: the 2D view's camera, in CSS pixels. */
export interface ScreenView {
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** `nodeHalves`' output. */
  readonly halves: Float32Array;
  readonly camera: { readonly x: number; readonly y: number; readonly scale: number };
  readonly viewport: { readonly width: number; readonly height: number };
  /** CSS pixels added around every node, for the smallest radius drawn and the antialiasing. */
  readonly pad: number;
}

/**
 * The shown nodes whose padded box meets the viewport, in index order (the order they paint
 * in). The quad pass draws only these: software rasterisers walk every instance of a draw.
 * ponytail: a fresh array per quad frame; pool it if a profile shows the collector.
 */
export function onScreen(view: ScreenView): Uint32Array {
  const { x, y, halves, camera, viewport, pad } = view;
  const count = Math.min(x.length, y.length, Math.floor(halves.length / 2));
  const out = new Uint32Array(count);
  let kept = 0;
  for (let node = 0; node < count; node += 1) {
    const halfX = halves[2 * node] ?? -1;
    if (halfX < 0) continue;
    const reachX = halfX * camera.scale + pad;
    const reachY = (halves[2 * node + 1] ?? 0) * camera.scale + pad;
    const screenX = (x[node] ?? 0) * camera.scale + camera.x;
    const screenY = (y[node] ?? 0) * camera.scale + camera.y;
    if (screenX + reachX < 0 || screenX - reachX > viewport.width) continue;
    if (screenY + reachY < 0 || screenY - reachY > viewport.height) continue;
    out[kept] = node;
    kept += 1;
  }
  return out.slice(0, kept);
}

/** `width` values per node of `column`, for the listed nodes only, in a column of the same type. */
export function gathered(column: Float32Array, nodes: Uint32Array, width: number): Float32Array;
export function gathered(column: Uint16Array, nodes: Uint32Array, width: number): Uint16Array;
export function gathered(column: Float32Array | Uint16Array, nodes: Uint32Array, width: number): Float32Array | Uint16Array {
  const length = nodes.length * width;
  const out = column instanceof Uint16Array ? new Uint16Array(length) : new Float32Array(length);
  for (let at = 0; at < nodes.length; at += 1) {
    const from = (nodes[at] ?? 0) * width;
    for (let lane = 0; lane < width; lane += 1) out[at * width + lane] = column[from + lane] ?? 0;
  }
  return out;
}
