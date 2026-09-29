/**
 * Arrow heads: one filled triangle per edge, pointing at its target and stopping at the
 * node's rim, batched into a single fill. The length is ARROW_LENGTH stroke widths, so a
 * thicker link carries a proportionally larger head.
 *
 * Ponytail: every edge is drawn source to target, because the snapshot carries no
 * direction flag: an undirected graph gets heads too. The heading of a bent edge is the
 * chord from its control point, and a routed edge (Curve, Polyline) points along the
 * straight line between its ends, not along its last segment.
 */
import { type Ends, bendOf, screenEnds, strokeWidth } from "./edges.ts";
import type { PaintCounts, PaintInput } from "./input.ts";

export const ARROW_LENGTH = 6;
const HALF_WIDTH = 0.5;

/** Below this the two ends coincide and there is no heading to point along. */
const MIN_CHORD = 1e-6;

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

export function paintArrows(input: PaintInput, counts: PaintCounts): void {
  const { ctx } = input;
  const length = strokeWidth(input) * ARROW_LENGTH;
  const ends: Ends = { ax: 0, ay: 0, bx: 0, by: 0 };
  ctx.fillStyle = input.theme.edge;
  ctx.globalAlpha = input.focus >= 0 ? input.theme.dimAlpha : 1;
  ctx.beginPath();
  for (let edge = 0; edge < input.frame.edgeCount; edge += 1) {
    const at = screenEnds(input, edge, ends);
    if (at !== null && headOf(input, edge, at, length)) counts.arrows += 1;
  }
  ctx.fill();
  counts.draws += 1;
  ctx.globalAlpha = 1;
  counts.arrowSize = counts.arrows > 0 ? length : 0;
}
