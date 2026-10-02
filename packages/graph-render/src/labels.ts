/**
 * Which labels are drawn this frame, and how opaque. Heaviest nodes first; a label shows
 * once the zoom passes its node's threshold, and only where no heavier label already sits.
 *
 * Ponytail: placement is greedy on a coarse screen grid, so a label is dropped when any
 * cell it touches is taken, even if the two texts would not have overlapped; and a width
 * that was never measured is estimated from the character count, which is wrong for wide
 * glyphs (CJK, emoji). Both err towards fewer labels. Hovering a node forces its label.
 */
import type { Camera, Viewport } from "./camera.ts";
import { fadeFactor } from "./labels2d/fade.ts";
import type { Style } from "./style.ts";

export interface LabelPolicy {
  /** The zoom at which a weightless node's label starts to appear. */
  readonly threshold: number;
  /** Labels per frame, forced ones included. */
  readonly budget: number;
  /** The text-fade slider, -3..3; absent is 0 (labels2d/fade.ts). */
  readonly fade?: number;
}

export interface LabelPlan {
  count: number;
  readonly node: Uint32Array;
  readonly alpha: Float32Array;
  /** Screen position of the label's top centre, in CSS pixels. */
  readonly x: Float32Array;
  readonly y: Float32Array;
}

export interface Occupancy {
  readonly columns: number;
  readonly rows: number;
  readonly taken: Uint8Array;
}

export interface LabelInput {
  readonly style: Style;
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** World distance from a node's centre to its lower edge. */
  readonly extent: Float32Array;
  readonly camera: Camera;
  readonly viewport: Viewport;
  /** When set, only nodes marked 1 are labelled, whatever the zoom. */
  readonly lit: Uint8Array | null;
  readonly policy: LabelPolicy;
  /** Measured width in CSS pixels, or 0 when the label was never drawn. */
  readonly widthOf: (node: number) => number;
  /** The sprite box height of a baked label, in CSS pixels (theme.labelHeight). */
  readonly height: number;
}

export const DEFAULT_POLICY: LabelPolicy = { threshold: 1.1, budget: 160 };
export const LABEL_HEIGHT = 16;
export const LABEL_GAP = 4;
const CELL_WIDTH = 32;
const CELL_HEIGHT = 16;
const FADE_SPAN = 0.4;
const WEIGHT_PULL = 0.8;
const ESTIMATED_GLYPH = 6.4;

export function newLabelPlan(budget: number): LabelPlan {
  return {
    count: 0,
    node: new Uint32Array(budget),
    alpha: new Float32Array(budget),
    x: new Float32Array(budget),
    y: new Float32Array(budget),
  };
}

export function occupancyFor(viewport: Viewport): Occupancy {
  const columns = Math.max(1, Math.ceil(viewport.width / CELL_WIDTH));
  const rows = Math.max(1, Math.ceil(viewport.height / CELL_HEIGHT));
  return { columns, rows, taken: new Uint8Array(columns * rows) };
}

/** 0 below the node's threshold, 1 once the zoom is `FADE_SPAN` past it. */
export function zoomAlpha(scale: number, weight: number, threshold: number): number {
  const own = threshold * (1 - WEIGHT_PULL * Math.sqrt(Math.min(1, Math.max(0, weight))));
  const t = Math.min(1, Math.max(0, (scale - own) / (own * FADE_SPAN)));
  return t * t * (3 - 2 * t);
}

function claimCells(occupancy: Occupancy, left: number, top: number, width: number): boolean {
  const fromColumn = Math.max(0, Math.floor(left / CELL_WIDTH));
  const toColumn = Math.min(occupancy.columns - 1, Math.floor((left + width) / CELL_WIDTH));
  const fromRow = Math.max(0, Math.floor(top / CELL_HEIGHT));
  const toRow = Math.min(occupancy.rows - 1, Math.floor((top + LABEL_HEIGHT) / CELL_HEIGHT));
  for (let row = fromRow; row <= toRow; row += 1) {
    for (let column = fromColumn; column <= toColumn; column += 1) {
      if (occupancy.taken[row * occupancy.columns + column] === 1) return false;
    }
  }
  for (let row = fromRow; row <= toRow; row += 1) {
    occupancy.taken.fill(1, row * occupancy.columns + fromColumn, row * occupancy.columns + toColumn + 1);
  }
  return true;
}

/**
 * Where a label's sprite box tops out. `below` is the studio's own: under the node's lower
 * edge and a gap down. `centred` is the SciGraphs overlay, whose text is centred on the
 * node (text_overlay.py:231-233, 545-556), and it keeps every label: the declutter that
 * drops the ones that collide (labels2d/declutter.ts) has already run, on the source's
 * own boxes, before the style reaches the painter.
 */
function topOf(input: LabelInput, node: number, sy: number): number {
  if (input.style.placement === "centred") return sy - input.height / 2;
  return sy + (input.extent[node] ?? 0) * input.camera.scale + LABEL_GAP;
}

function offscreen(input: LabelInput, sx: number, top: number): boolean {
  return sx < 0 || sx > input.viewport.width || top < -input.height || top > input.viewport.height;
}

function push(plan: LabelPlan, node: number, alpha: number, at: { readonly x: number; readonly y: number }): void {
  const index = plan.count;
  plan.node[index] = node;
  plan.alpha[index] = alpha;
  plan.x[index] = at.x;
  plan.y[index] = at.y;
  plan.count = index + 1;
}

function place(input: LabelInput, node: number, alpha: number, out: { plan: LabelPlan; occupancy: Occupancy }): void {
  const text = input.style.labels[node];
  if (text === undefined || text === "" || input.style.hidden?.[node] === 1) return;
  const { camera } = input;
  // The x test runs first: a zoomed-in rank scan rejects most nodes here, before any other read.
  const sx = (input.x[node] ?? 0) * camera.scale + camera.x;
  if (sx < 0 || sx > input.viewport.width) return;
  const top = topOf(input, node, (input.y[node] ?? 0) * camera.scale + camera.y);
  if (offscreen(input, sx, top)) return;
  const keep = input.style.placement === "centred";
  // A focus's labels are forced: the neighbourhood is named even where two texts touch.
  if (!keep && input.lit === null) {
    const width = input.widthOf(node) || text.length * ESTIMATED_GLYPH + 8;
    if (!claimCells(out.occupancy, sx - width / 2, top, width)) return;
  }
  push(out.plan, node, keep ? 1 : alpha, { x: sx, y: top });
}

/** A planned label's opacity at the current zoom: forced and centred labels are opaque. */
function alphaOf(input: LabelInput, node: number): number {
  if (input.lit !== null || input.style.placement === "centred") return 1;
  const threshold = input.policy.threshold * fadeFactor(input.policy.fade ?? 0);
  return zoomAlpha(input.camera.scale, input.style.weights[node] ?? 0, threshold);
}

/**
 * Moves the last plan's labels with the camera instead of laying them out again: O(plan)
 * where planLabels scans the rank order, which a zoomed-in million-node view walks to the end
 * (about 7 ms a frame, target/p5-zoom.log). A label that leaves the screen or fades out is
 * dropped; none is added.
 *
 * Caveat: nothing claims the grid, so a zoom-out can draw two labels over each other, and a
 * label a zoom-in or a pan would bring into view is missing. Both last until the next full
 * plan, which the loop runs once the view settles.
 */
export function followLabels(input: LabelInput, plan: LabelPlan): void {
  const followed = plan.count;
  plan.count = 0;
  const { camera } = input;
  for (let at = 0; at < followed; at += 1) {
    const node = plan.node[at] ?? 0;
    const alpha = alphaOf(input, node);
    const sx = (input.x[node] ?? 0) * camera.scale + camera.x;
    const top = topOf(input, node, (input.y[node] ?? 0) * camera.scale + camera.y);
    if (alpha > 0.02 && !offscreen(input, sx, top)) push(plan, node, alpha, { x: sx, y: top });
  }
}

export function planLabels(input: LabelInput, plan: LabelPlan, occupancy: Occupancy): void {
  plan.count = 0;
  occupancy.taken.fill(0);
  const out = { plan, occupancy };
  const budget = Math.min(input.policy.budget, plan.node.length);
  const { rank, weights } = input.style;
  // A SciGraphs overlay is a figure's own label set: the zoom fade is the studio's
  // affordance and the source has none, so a centred label is drawn at full opacity
  // whatever the scale, and the threshold never culls one.
  const centred = input.style.placement === "centred";
  const factor = fadeFactor(input.policy.fade ?? 0);
  // Fading later than the default is the one thing that does cull a centred label, and it
  // culls the whole set together: the source has no per-label weight to rank by.
  if (centred && input.lit === null && input.camera.scale < input.policy.threshold * (factor - 1)) return;
  for (let at = 0; at < rank.length && plan.count < budget; at += 1) {
    const node = rank[at] ?? 0;
    if (input.lit !== null) {
      if (input.lit[node] === 1) place(input, node, 1, out);
      continue;
    }
    const alpha = centred ? 1 : zoomAlpha(input.camera.scale, weights[node] ?? 0, input.policy.threshold * factor);
    // Rank is by weight, so every node after the first invisible one is invisible too.
    if (alpha <= 0.02) break;
    place(input, node, alpha, out);
  }
}
