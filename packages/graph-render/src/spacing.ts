/**
 * The spacing most nodes of a drawing have to their neighbours, read from how crowded a
 * uniform grid over them is. O(n) time and memory and no randomness: `frame.ts` reads it once
 * per layout run, so the same columns always give the same number.
 */

/** Nodes per cell of a uniform spread: the grid is fine enough to see a clump, coarse enough to count. */
const PER_CELL = 16;
/** A cell holding more nodes than this is a clump, and is gridded again over its own box. */
const CROWDED = 64;
/** How many times a clump is gridded again; past it a crowded cell keeps its own count. */
const MAX_DEPTH = 4;
/** The median is taken over at most this many nodes, spread evenly through node order. */
const SAMPLE = 4096;

interface Columns {
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** Each node's local spacing, written once by the group that settles it. */
  readonly s: Float64Array;
}

interface Grid {
  readonly minX: number;
  readonly minY: number;
  readonly side: number;
  readonly cols: number;
  readonly rows: number;
  /** A line spaces its nodes by `side / occ`, a plane by `side / sqrt(occ)`. */
  readonly linear: boolean;
}

/** One gridding of a group: its node indices, the cell of each, and the nodes per cell. */
interface Pass {
  readonly members: Uint32Array;
  readonly cells: Uint32Array;
  readonly counts: Uint32Array;
  readonly depth: number;
}

/**
 * The grid over a group's box, or `null` when the box is a point or not finite. A box thinner
 * than 1 / (PER_CELL * m) of its length is a line: gridded as a plane, it would need more than
 * m cells along its length, and a count in O(m) memory is the point of the grid.
 */
function gridOf(columns: Columns, members: Uint32Array): Grid | null {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const i of members) {
    const px = columns.x[i] ?? 0;
    const py = columns.y[i] ?? 0;
    if (px < minX) minX = px;
    if (px > maxX) maxX = px;
    if (py < minY) minY = py;
    if (py > maxY) maxY = py;
  }
  const w = maxX - minX;
  const h = maxY - minY;
  const long = Math.max(w, h);
  if (!(long > 0 && Number.isFinite(long))) return null;
  const m = members.length;
  const linear = Math.min(w, h) * PER_CELL * m < long;
  const side = linear ? (long * PER_CELL) / m : Math.sqrt((w * h * PER_CELL) / m);
  const cols = linear && w < h ? 1 : Math.max(1, Math.ceil(w / side));
  const rows = linear && w >= h ? 1 : Math.max(1, Math.ceil(h / side));
  return { minX, minY, side, cols, rows, linear };
}

/** The cell index along one axis; a node on the max edge (or a NaN) clamps into the grid. */
function cellAlong(value: number, min: number, side: number, count: number): number {
  const at = Math.floor((value - min) / side);
  if (at >= count) return count - 1;
  return at > 0 ? at : 0;
}

function passOf(columns: Columns, members: Uint32Array, grid: Grid, depth: number): Pass {
  const cells = new Uint32Array(members.length);
  const counts = new Uint32Array(grid.cols * grid.rows);
  members.forEach((i, at) => {
    const col = cellAlong(columns.x[i] ?? 0, grid.minX, grid.side, grid.cols);
    const row = cellAlong(columns.y[i] ?? 0, grid.minY, grid.side, grid.rows);
    const cell = row * grid.cols + col;
    cells[at] = cell;
    counts[cell] = (counts[cell] ?? 0) + 1;
  });
  return { members, cells, counts, depth };
}

/** Each crowded cell's members, gathered by a counting sort, gridded again over their own box. */
function regroup(columns: Columns, pass: Pass): void {
  const { members, cells, counts } = pass;
  const start = new Uint32Array(counts.length + 1);
  for (let cell = 0; cell < counts.length; cell += 1) start[cell + 1] = (start[cell] ?? 0) + (counts[cell] ?? 0);
  const next = start.slice(0, counts.length);
  const order = new Uint32Array(members.length);
  for (let at = 0; at < members.length; at += 1) {
    const cell = cells[at] ?? 0;
    const slot = next[cell] ?? 0;
    order[slot] = members[at] ?? 0;
    next[cell] = slot + 1;
  }
  for (let cell = 0; cell < counts.length; cell += 1) {
    if ((counts[cell] ?? 0) <= CROWDED) continue;
    spaceGroup(columns, order.subarray(start[cell] ?? 0, start[cell + 1] ?? 0), pass.depth + 1);
  }
}

/** Writes the local spacing of every node of a group; a group whose box is a point keeps 0. */
function spaceGroup(columns: Columns, members: Uint32Array, depth: number): void {
  const grid = gridOf(columns, members);
  if (grid === null) return;
  const pass = passOf(columns, members, grid, depth);
  let crowded = 0;
  for (let at = 0; at < members.length; at += 1) {
    const occ = pass.counts[pass.cells[at] ?? 0] ?? 1;
    if (occ > CROWDED && depth < MAX_DEPTH) crowded += 1;
    else columns.s[members[at] ?? 0] = grid.side / (grid.linear ? occ : Math.sqrt(occ));
  }
  if (crowded > 0) regroup(columns, pass);
}

/** The median of a stride sample of `s`; the upper middle when the sample is even. */
function sampledMedian(s: Float64Array): number {
  const size = Math.min(s.length, SAMPLE);
  const sample = new Float64Array(size);
  for (let k = 0; k < size; k += 1) sample[k] = s[Math.floor((k * s.length) / size)] ?? 0;
  sample.sort((a, b) => a - b);
  return sample[size >> 1] ?? 0;
}

/**
 * The spacing most nodes have to their neighbours, in motor units; 0 when it cannot be told.
 *
 * Ponytail: the median serves the majority, so a clump holding under half the nodes can still
 * overlap (it reads as an outlier); x and y only, so a 3D drawing seen edge-on can overlap; a
 * k-wide lattice reads (k - 1) / k of its pitch; a near-line drops its thin extent. Zooming in
 * is the escape hatch.
 */
export function typicalSpacing(x: Float32Array, y: Float32Array): number {
  const n = x.length;
  if (n < 2) return 0;
  const columns = { x, y, s: new Float64Array(n) };
  spaceGroup(columns, Uint32Array.from({ length: n }, (_, i) => i), 0);
  const median = sampledMedian(columns.s);
  return median > 0 && Number.isFinite(median) ? median : 0;
}
