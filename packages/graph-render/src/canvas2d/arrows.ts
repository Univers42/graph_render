/**
 * Arrow heads: one filled triangle per edge, pointing at its target and stopping at the
 * node's rim, batched into a single fill. The length is ARROW_LENGTH stroke widths, so a
 * thicker link carries a proportionally larger head.
 *
 * In the gradient mode a head takes the colour of the node it points at, so the heads are
 * batched per palette slot instead of in one fill; the flat mode is one fill as it always was.
 *
 * Ponytail: every edge is drawn source to target, because the snapshot carries no
 * direction flag: an undirected graph gets heads too. The heading of a bent edge is the
 * chord from its control point, and a routed edge (Curve, Polyline) points along the
 * straight line between its ends, not along its last segment.
 */
import { type Ends, bendOf, screenEnds, strokeWidth } from "./edges.ts";
import type { EdgePlan } from "./edgeGradient.ts";
import { slotOf } from "./edgeGradient.ts";
import type { PaintCounts, PaintInput } from "./input.ts";

export const ARROW_LENGTH = 6;
const HALF_WIDTH = 0.5;

/** Below this the two ends coincide and there is no heading to point along. */
const MIN_CHORD = 1e-6;

/** The one pass's own scratch: where a head is traced and how long it is. */
interface Batch {
  readonly input: PaintInput;
  readonly counts: PaintCounts;
  readonly ends: Ends;
  readonly length: number;
}

function headOf(input: PaintInput, edge: number, ends: Ends, length: number): boolean {
  const bend = bendOf(input, ends);
  const fromX = bend === null ? ends.ax : bend.x;
  const fromY = bend === null ? ends.ay : bend.y;
  const dx = ends.bx - fromX;
  const dy = ends.by - fromY;
  const chord = Math.hypot(dx, dy);
  if (chord < MIN_CHORD) return false;
  const ux = dx / chord;
  const uy = dy / chord;
  const rim = (input.extent[input.frame.target[edge] ?? 0] ?? 0) * input.camera.scale;
  const tipX = ends.bx - ux * rim;
  const tipY = ends.by - uy * rim;
  const half = length * HALF_WIDTH;
  const { ctx } = input;
  ctx.moveTo(tipX, tipY);
  ctx.lineTo(tipX - ux * length - uy * half, tipY - uy * length + ux * half);
  ctx.lineTo(tipX - ux * length + uy * half, tipY - uy * length - ux * half);
  ctx.lineTo(tipX, tipY);
  return true;
}

/** One head traced into the open path, or no head when the edge is culled. */
function head(batch: Batch, edge: number): void {
  const { input, counts, ends, length } = batch;
  const at = screenEnds(input, edge, ends);
  if (at !== null && headOf(input, edge, at, length)) counts.arrows += 1;
}

/** The edges grouped by the slot of their target, slot-major and frame order inside. */
function byTarget(input: PaintInput, size: number): { readonly start: Uint32Array; readonly edges: Uint32Array } {
  const count = input.frame.edgeCount;
  const start = new Uint32Array(size + 1);
  for (let edge = 0; edge < count; edge += 1) {
    const slot = slotOf(input, input.frame.target[edge] ?? 0);
    start[slot + 1] = (start[slot + 1] ?? 0) + 1;
  }
  for (let slot = 0; slot < size; slot += 1) start[slot + 1] = (start[slot + 1] ?? 0) + (start[slot] ?? 0);
  const next = start.slice(0, size);
  const edges = new Uint32Array(count);
  for (let edge = 0; edge < count; edge += 1) {
    const slot = slotOf(input, input.frame.target[edge] ?? 0);
    edges[next[slot] ?? 0] = edge;
    next[slot] = (next[slot] ?? 0) + 1;
  }
  return { start, edges };
}

/** One fill per palette slot of the targets: a head wears the colour of the node it points at. */
function paintBySlot(batch: Batch, plan: EdgePlan): void {
  const { input, counts } = batch;
  const { start, edges } = byTarget(input, plan.palette.length);
  for (let slot = 0; slot < plan.palette.length; slot += 1) {
    const from = start[slot] ?? 0;
    const to = start[slot + 1] ?? 0;
    if (to <= from) continue;
    input.ctx.fillStyle = input.style.palette[slot] ?? "#9a9a9a";
    input.ctx.beginPath();
    for (let at = from; at < to; at += 1) head(batch, edges[at] ?? 0);
    input.ctx.fill();
    counts.draws += 1;
    counts.arrowFills += 1;
  }
}

export function paintArrows(input: PaintInput, counts: PaintCounts, plan: EdgePlan | null): void {
  const batch: Batch = { input, counts, ends: { ax: 0, ay: 0, bx: 0, by: 0 }, length: strokeWidth(input) * ARROW_LENGTH };
  input.ctx.globalAlpha = input.focus >= 0 ? input.theme.dimAlpha : 1;
  if (plan !== null) paintBySlot(batch, plan);
  else {
    input.ctx.fillStyle = input.theme.edge;
    input.ctx.beginPath();
    for (let edge = 0; edge < input.frame.edgeCount; edge += 1) head(batch, edge);
    input.ctx.fill();
    counts.draws += 1;
    counts.arrowFills += 1;
  }
  input.ctx.globalAlpha = 1;
  counts.arrowSize = counts.arrows > 0 ? batch.length : 0;
}