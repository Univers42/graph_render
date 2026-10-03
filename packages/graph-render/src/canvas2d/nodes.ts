/**
 * Nodes, batched by colour: one path and one fill per palette entry. A node smaller than
 * DOT_RADIUS on screen is a square — at that size the eye cannot tell, and a rect costs
 * the rasteriser less than an arc.
 *
 * A SciGraphs look carries a base colour per palette entry instead, and then every node is
 * a lit sphere baked once and blitted (sprite/impostor.ts).
 *
 * Ponytail: one blit per node is a drawImage each, and the sprite cache bakes at most 32 a
 * frame (canvas2d/impostors.ts:16), so the impostor pass is only taken while a scene fits
 * inside IMPOSTOR_BUDGET; past it the batched fills are the drawing and the sphere shading
 * is the detail given up. The escape hatch is to raise the budget, at one blit per node.
 */
import type { PaintCounts, PaintInput } from "./input.ts";

export const MIN_SCREEN_RADIUS = 1.25;
const DOT_RADIUS = 1.75;
const TAU = Math.PI * 2;
const RING_GAP = 3;

/** Past this many nodes a frame cannot bake a sphere for each of them in one go. */
export const IMPOSTOR_BUDGET = 4096;

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

/** The world radius of a node on screen, floored at the smallest dot worth drawing. */
function screenRadius(input: PaintInput, node: number): number {
  return Math.max(MIN_SCREEN_RADIUS, (input.extent[node] ?? 0) * input.camera.scale);
}

/** The sprite size a sphere is baked at, in device pixels: the drawn diameter. */
function sphereSize(input: PaintInput, node: number): number {
  return Math.ceil(screenRadius(input, node) * 2 * input.dpr);
}

function paintSphere(input: PaintInput, node: number): boolean {
  const { style } = input;
  const bases = style.spheres;
  if (bases === null) return false;
  const base = bases[style.colours[node] ?? 0];
  const size = sphereSize(input, node);
  const sprite = base === undefined ? null : input.sprites.sphere(base, size);
  if (sprite === null) return false;
  const { camera } = input;
  const sx = (input.x[node] ?? 0) * camera.scale + camera.x;
  const sy = (input.y[node] ?? 0) * camera.scale + camera.y;
  const width = size / input.dpr;
  input.ctx.drawImage(sprite.image, sx - width / 2, sy - width / 2, width, width);
  return true;
}

function paintSpheres(input: PaintInput, counts: PaintCounts, pass: Pass): void {
  input.ctx.globalAlpha = pass === "dim" ? input.theme.dimAlpha : 1;
  for (let node = 0; node < input.frame.nodeCount; node += 1) {
    if (skipped(input, node, pass)) continue;
    if (!paintSphere(input, node)) continue;
    counts.nodes += 1;
    counts.draws += 1;
  }
}

/**
 * True when this scene is drawn as lit spheres rather than as batched fills.
 *
 * A `Box` frame never takes the impostor path: the sprite the cache hands back is a lit disc
 * of one radius (`sphereSize` squares the drawn diameter) and there is no rect impostor baked,
 * so blitting one would spend the node's `w`, its `h` and the rim stroke and turn every box
 * into a round sprite. A Box frame keeps the shape the snapshot gave it and is drawn by the
 * flat rect path — the same path a Box scene past IMPOSTOR_BUDGET already takes.
 */
export function impostorOf(input: PaintInput): boolean {
  if (input.frame.nodeKind === "Box") return false;
  return input.style.spheres !== null && input.frame.nodeCount <= IMPOSTOR_BUDGET;
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
  if (impostorOf(input)) {
    // The spheres are drawn in the same two passes as the flat fills, so a lit
    // neighbourhood dims the rest of a look-driven scene exactly as it dims the studio's.
    if (input.focus >= 0) {
      paintSpheres(input, counts, "dim");
      paintSpheres(input, counts, "lit");
    } else {
      paintSpheres(input, counts, "all");
    }
  } else if (input.focus >= 0) {
    paintPass(input, counts, "dim");
    paintPass(input, counts, "lit");
  } else {
    paintPass(input, counts, "all");
  }
  paintRings(input);
}

function paintRings(input: PaintInput): void {
  input.ctx.globalAlpha = 1;
  paintRing(input, input.selected, 2);
  if (input.focus !== input.selected) paintRing(input, input.focus, 1.5);
}

/**
 * The lit neighbourhood at full strength and the rings, over a GPU layer that drew every
 * node. Caveat: it walks every node to find the lit ones, as the 2D lit pass does; a
 * million-node scene pays that walk on every frame a focus is shown.
 */
export function paintLitNodes(input: PaintInput, counts: PaintCounts): void {
  if (input.focus >= 0) paintPass(input, counts, "lit");
  paintRings(input);
}
