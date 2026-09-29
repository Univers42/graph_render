/**
 * Which node is under the pointer: a uniform grid over node centres, built in O(n) per
 * frame and read in O(nodes in the cells the query touches).
 */
import type { Bounds } from "./camera.ts";

export interface Grid {
  readonly minX: number;
  readonly minY: number;
  readonly cell: number;
  readonly columns: number;
  readonly rows: number;
  /** Cell `c` owns `items[start[c]..start[c+1]]`. */
  readonly start: Uint32Array;
  readonly items: Uint32Array;
}

export interface Positions {
  readonly x: Float32Array;
  readonly y: Float32Array;
}

export interface PickQuery {
  readonly x: number;
  readonly y: number;
  /** World distance past a node's own extent that still counts as a hit. */
  readonly tolerance: number;
  /** The largest node extent in the frame: how far a centre can be from a hit. */
  readonly reach: number;
  /** Signed distance from the node's edge to the point; ≤ tolerance is a hit. */
  readonly distance: (node: number, x: number, y: number) => number;
}

const MAX_CELLS = 1 << 20;

function cellOf(grid: Grid, x: number, y: number): number {
  const column = Math.min(grid.columns - 1, Math.max(0, Math.floor((x - grid.minX) / grid.cell)));
  const row = Math.min(grid.rows - 1, Math.max(0, Math.floor((y - grid.minY) / grid.cell)));
  return row * grid.columns + column;
}

function shapeOf(bounds: Bounds, count: number): Pick<Grid, "cell" | "columns" | "rows"> {
  const width = Math.max(1, bounds.maxX - bounds.minX);
  const height = Math.max(1, bounds.maxY - bounds.minY);
  // About two nodes a cell on a uniform spread.
  const cell = Math.max(1, Math.sqrt((width * height * 2) / Math.max(1, count)));
  const columns = Math.max(1, Math.ceil(width / cell));
  const rows = Math.max(1, Math.ceil(height / cell));
  if (columns * rows <= MAX_CELLS) return { cell, columns, rows };
  // A near-collinear frame: one long axis would ask for millions of empty cells.
  const grown = cell * Math.sqrt((columns * rows) / MAX_CELLS);
  return { cell: grown, columns: Math.max(1, Math.ceil(width / grown)), rows: Math.max(1, Math.ceil(height / grown)) };
}

export function gridOf(positions: Positions, bounds: Bounds | null): Grid {
  const count = positions.x.length;
  const box = bounds ?? { minX: 0, minY: 0, maxX: 1, maxY: 1 };
  const shape = shapeOf(box, count);
  const empty = { minX: box.minX, minY: box.minY, ...shape, start: new Uint32Array(0), items: new Uint32Array(0) };
  const start = new Uint32Array(shape.columns * shape.rows + 1);
  for (let i = 0; i < count; i += 1) {
    const cell = cellOf(empty, positions.x[i] ?? 0, positions.y[i] ?? 0) + 1;
    start[cell] = (start[cell] ?? 0) + 1;
  }
  for (let c = 1; c < start.length; c += 1) start[c] = (start[c] ?? 0) + (start[c - 1] ?? 0);
  const next = start.slice(0, start.length - 1);
  const items = new Uint32Array(count);
  for (let i = 0; i < count; i += 1) {
    const cell = cellOf(empty, positions.x[i] ?? 0, positions.y[i] ?? 0);
    const at = next[cell] ?? 0;
    items[at] = i;
    next[cell] = at + 1;
  }
  return { ...empty, start, items };
}

/** The node whose edge is nearest the point, within the tolerance; -1 for none. */
export function pickNode(grid: Grid, query: PickQuery): number {
  const span = query.reach + query.tolerance;
  const low = cellOf(grid, query.x - span, query.y - span);
  const high = cellOf(grid, query.x + span, query.y + span);
  const lowColumn = low % grid.columns;
  const highColumn = high % grid.columns;
  let best = -1;
  let bestDistance = query.tolerance;
  for (let row = Math.floor(low / grid.columns); row <= Math.floor(high / grid.columns); row += 1) {
    const from = grid.start[row * grid.columns + lowColumn] ?? 0;
    const to = grid.start[row * grid.columns + highColumn + 1] ?? 0;
    for (let at = from; at < to; at += 1) {
      const node = grid.items[at] ?? 0;
      const distance = query.distance(node, query.x, query.y);
      // A tie goes to the later index: the node painted last is the one on top.
      if (distance < bestDistance || (distance === bestDistance && node > best)) {
        best = node;
        bestDistance = distance;
      }
    }
  }
  return best;
}
