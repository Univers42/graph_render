/**
 * The spacing most nodes of a drawing have to their neighbours: how crowded a uniform grid over
 * them is, capped by how close each cell's own members stand. A cell whose nodes lie on a few
 * lines — lanes, Sugiyama layers — reads its pitch, not the width it happens to fill. O(n) time
 * and memory and no randomness: `frame.ts` reads it once per layout run, so the same columns
 * always give the same number.
 */

/** Nodes per cell of a uniform spread: the grid is fine enough to see a clump, coarse enough to count. */
const PER_CELL = 16;
/** A cell holding more nodes than this is a clump, and is gridded again over its own box. */
const CROWDED = 64;
/** How many times a clump is gridded again; past it a crowded cell keeps its own count. */
const MAX_DEPTH = 4;
/** The median is taken over at most this many nodes, spread evenly through node order. */
const SAMPLE = 4096;
/**
 * How many median nearest-neighbour distances the cell estimate may be cut down to.
 *
 * For a uniform spread of density ρ the median nearest distance is sqrt(ln 2 / π) / sqrt(ρ), about
 * 0.47 / √ρ, while the cell estimate reads side / sqrt(occ) ≈ 0.97 / √ρ. 2.5 × 0.47 = 1.17 sits
 * above 0.97, so a uniform spread and a lattice keep today's number exactly; only a cell that is
 * denser inside than it looks is cut, and a within-cell search can only read high (a neighbour
 * across the border is missed), which is the safe direction.
 */
const NEAREST_RATIO = 2.5;

interface Columns {
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** Each node's local spacing, written once by the group that settles it. */
  readonly s: Float64Array;
  /** Each node's distance to its nearest neighbour in its own cell; `Infinity` where there is none. */
  readonly d: Float64Array;
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

/** A pass's members gathered by cell: cell `c` at `order[start[c] .. start[c + 1]]`. */
interface Grouped {
  readonly start: Uint32Array;
  readonly order: Uint32Array;
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

/** The pass's members gathered by cell, by a counting sort: no hash, no per-cell array. */
function sortByCell(pass: Pass): Grouped {
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
  return { start, order };
}

/**
 * Each member of one cell's distance to its nearest other member of that cell, by brute force:
 * at most CROWDED evaluations per node. Distance 0 is a coincident node, not a neighbour, so it
 * is skipped and a cell of coincident nodes keeps `Infinity`.
 */
function nearestInCell(columns: Columns, grouped: Grouped, cell: number): void {
  const from = grouped.start[cell] ?? 0;
  const to = grouped.start[cell + 1] ?? 0;
  for (let a = from; a < to; a += 1) {
    const i = grouped.order[a] ?? 0;
    const px = columns.x[i] ?? 0;
    const py = columns.y[i] ?? 0;
    let best = Infinity;
    for (let b = from; b < to; b += 1) {
      if (b === a) continue;
      const j = grouped.order[b] ?? 0;
      const dx = (columns.x[j] ?? 0) - px;
      const dy = (columns.y[j] ?? 0) - py;
      const gap = Math.sqrt(dx * dx + dy * dy);
      if (gap > 0 && gap < best) best = gap;
    }
    columns.d[i] = best;
  }
}

/** Every cell holding 2 to CROWDED nodes, read as its members' nearest neighbours. */
function nearestOfPass(columns: Columns, pass: Pass, grouped: Grouped): void {
  for (let cell = 0; cell < pass.counts.length; cell += 1) {
    const occ = pass.counts[cell] ?? 0;
    if (occ >= 2 && occ <= CROWDED) nearestInCell(columns, grouped, cell);
  }
}

/** Each crowded cell's members, gridded again over their own box. */
function regroup(columns: Columns, pass: Pass, grouped: Grouped): void {
  const { counts } = pass;
  for (let cell = 0; cell < counts.length; cell += 1) {
    if ((counts[cell] ?? 0) <= CROWDED) continue;
    const members = grouped.order.subarray(grouped.start[cell] ?? 0, grouped.start[cell + 1] ?? 0);
    spaceGroup(columns, members, pass.depth + 1);
  }
}

/** Writes the local spacing and the nearest distance of every node of a group; a point box keeps 0. */
function spaceGroup(columns: Columns, members: Uint32Array, depth: number): void {
  const grid = gridOf(columns, members);
  if (grid === null) return;
  const pass = passOf(columns, members, grid, depth);
  const grouped = sortByCell(pass);
  let crowded = 0;
  for (let at = 0; at < members.length; at += 1) {
    const occ = pass.counts[pass.cells[at] ?? 0] ?? 1;
    if (occ > CROWDED && depth < MAX_DEPTH) crowded += 1;
    else columns.s[members[at] ?? 0] = grid.side / (grid.linear ? occ : Math.sqrt(occ));
  }
  nearestOfPass(columns, pass, grouped);
  if (crowded > 0) regroup(columns, pass, grouped);
}

/** The median of a stride sample of a column; the upper middle when the sample is even. */
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
 * k-wide lattice reads (k - 1) / k of its pitch; a near-line drops its thin extent. A drawing of
 * far-apart pairs, every node with one close twin, reads NEAREST_RATIO times the pair distance and
 * is spread that far out. The nearest search stays inside a cell, so it misses neighbours across a
 * border. Zooming in is the escape hatch.
 */
export function typicalSpacing(x: Float32Array, y: Float32Array): number {
  const n = x.length;
  if (n < 2) return 0;
  const columns = { x, y, s: new Float64Array(n), d: new Float64Array(n).fill(Infinity) };
  spaceGroup(columns, Uint32Array.from({ length: n }, (_, i) => i), 0);
  const cell = sampledMedian(columns.s);
  if (!(cell > 0 && Number.isFinite(cell))) return 0;
  return Math.min(cell, NEAREST_RATIO * sampledMedian(columns.d));
}
