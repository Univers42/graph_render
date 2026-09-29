/**
 * Nodes, batched by colour: one path and one fill per palette entry. A node smaller than
 * DOT_RADIUS on screen is a square — at that size the eye cannot tell, and a rect costs
 * the rasteriser less than an arc.
 */
import type { PaintCounts, PaintInput } from "./input.ts";

export const MIN_SCREEN_RADIUS = 1.25;
const DOT_RADIUS = 1.75;
const TAU = Math.PI * 2;
const RING_GAP = 3;

/** Which nodes a pass draws: all of them, or one side of the lit neighbourhood. */
type Pass = "all" | "dim" | "lit";

function skipped(input: PaintInput, node: number, pass: Pass): boolean {
  if (input.style.hidden?.[node] === 1) return true;
  if (pass === "all") return false;
  return (input.lit[node] === 1) !== (pass === "lit");
}

function traceBox(input: PaintInput, node: number): boolean {
  const { ctx, camera, frame, viewport } = input;
  const w = (frame.w?.[node] ?? 0) * camera.scale;
  const h = (frame.h?.[node] ?? 0) * camera.scale;
  const left = (input.x[node] ?? 0) * camera.scale + camera.x - w / 2;
  const top = (input.y[node] ?? 0) * camera.scale + camera.y - h / 2;
  if (left > viewport.width || top > viewport.height || left + w < 0 || top + h < 0) return false;
  ctx.rect(left, top, Math.max(w, 1), Math.max(h, 1));
  return true;
}

function traceDisc(input: PaintInput, node: number): boolean {
  const { ctx, camera, viewport } = input;
  const radius = Math.max(MIN_SCREEN_RADIUS, (input.extent[node] ?? 0) * camera.scale);
  const sx = (input.x[node] ?? 0) * camera.scale + camera.x;
  const sy = (input.y[node] ?? 0) * camera.scale + camera.y;
  if (sx < -radius || sy < -radius || sx > viewport.width + radius || sy > viewport.height + radius) return false;
  if (radius < DOT_RADIUS) {
    ctx.rect(sx - radius, sy - radius, radius * 2, radius * 2);
    return true;
  }
  ctx.moveTo(sx + radius, sy);
  ctx.arc(sx, sy, radius, 0, TAU);
  return true;
}

function paintBucket(input: PaintInput, bucket: number, pass: Pass): number {
  const { ctx, style } = input;
  const boxes = input.frame.nodeKind === "Box";
  const end = style.bucketStart[bucket + 1] ?? 0;
  let drawn = 0;
  ctx.beginPath();
  for (let at = style.bucketStart[bucket] ?? 0; at < end; at += 1) {
    const node = style.bucketItems[at] ?? 0;
    if (skipped(input, node, pass)) continue;
    if (boxes ? traceBox(input, node) : traceDisc(input, node)) drawn += 1;
  }
  if (drawn === 0) return 0;
  ctx.fillStyle = style.palette[bucket] ?? "#9a9a9a";
  ctx.fill();
  if (boxes) {
    ctx.strokeStyle = input.theme.rim;
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  return drawn;
}

function paintPass(input: PaintInput, counts: PaintCounts, pass: Pass): void {
  input.ctx.globalAlpha = pass === "dim" ? input.theme.dimAlpha : 1;
  for (let bucket = 0; bucket < input.style.palette.length; bucket += 1) {
    const drawn = paintBucket(input, bucket, pass);
    counts.nodes += drawn;
    if (drawn > 0) counts.draws += 1;
  }
}

function paintRing(input: PaintInput, node: number, width: number): void {
  if (node < 0 || node >= input.frame.nodeCount || input.style.hidden?.[node] === 1) return;
  const { ctx, camera } = input;
  const radius = Math.max(MIN_SCREEN_RADIUS, (input.extent[node] ?? 0) * camera.scale) + RING_GAP;
  ctx.beginPath();
  ctx.arc((input.x[node] ?? 0) * camera.scale + camera.x, (input.y[node] ?? 0) * camera.scale + camera.y, radius, 0, TAU);
  ctx.strokeStyle = input.theme.ring;
  ctx.lineWidth = width;
  ctx.stroke();
}

export function paintNodes(input: PaintInput, counts: PaintCounts): void {
  if (input.focus >= 0) {
    paintPass(input, counts, "dim");
    paintPass(input, counts, "lit");
  } else {
    paintPass(input, counts, "all");
  }
  input.ctx.globalAlpha = 1;
  paintRing(input, input.selected, 2);
  if (input.focus !== input.selected) paintRing(input, input.focus, 1.5);
}
