/** One frame on a 2D context: background, edges, nodes, labels — in that order. */
import { type PaintCounts, type PaintInput, newCounts } from "./input.ts";
import { paintEdges, paintLitEdges } from "./edges.ts";
import { paintGlow } from "./glow.ts";
import { paintLitNodes, paintNodes } from "./nodes.ts";
import { paint3d } from "../three/paint3d.ts";

function paintLabels(input: PaintInput, counts: PaintCounts): void {
  const { ctx, labels, sprites, style } = input;
  for (let at = 0; at < labels.count; at += 1) {
    const sprite = sprites.get(style.labels[labels.node[at] ?? 0] ?? "");
    if (sprite === null) continue;
    ctx.globalAlpha = labels.alpha[at] ?? 1;
    // Whole pixels: a sprite blitted at a fraction is resampled and the text blurs.
    const left = Math.round((labels.x[at] ?? 0) - sprite.width / 2);
    ctx.drawImage(sprite.image, left, Math.round(labels.y[at] ?? 0), sprite.width, sprite.height);
    counts.labels += 1;
  }
  ctx.globalAlpha = 1;
}

/**
 * The ground, painted inside the frame the view already draws: no timer, no frame request.
 * Ponytail: the aurora is one diagonal three-stop gradient, not an animated field; it looks
 * flat on a very small viewport, and its corners are the first and last stop, not blends.
 */
function paintGround(input: PaintInput): void {
  const { ctx, theme, viewport } = input;
  const backdrop = theme.backdrop;
  if (backdrop?.mode === "aurora") {
    const gradient = ctx.createLinearGradient(0, 0, viewport.width, viewport.height);
    backdrop.stops.forEach((colour, at) => gradient.addColorStop(at / 2, colour));
    ctx.fillStyle = gradient;
  } else {
    ctx.fillStyle = backdrop?.mode === "flat" ? backdrop.colour : theme.background;
  }
  ctx.fillRect(0, 0, viewport.width, viewport.height);
}

export function paintFrame(input: PaintInput): PaintCounts {
  // A frame with a z column is a 3D drawing and goes to its own painter whole: it projects,
  // it sorts and it fills per node, none of which the 2D passes below can express. It
  // returns here, so a 2D frame is drawn by exactly the code that has always drawn it.
  if (input.space !== null && input.space !== undefined) return paint3d(input, input.space, newCounts());
  const counts = newCounts();
  const { ctx, dpr } = input;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalAlpha = 1;
  paintGround(input);
  if (input.bulk?.(input, counts) === true) {
    // The GPU layer drew every edge and node, dimmed under a focus: the lit neighbourhood
    // and the rings go over it at full strength, and are not counted a second time.
    const drawn = counts.nodes;
    if (input.focus >= 0) paintLitEdges(input, counts);
    paintLitNodes(input, counts);
    counts.nodes = drawn;
  } else {
    paintEdges(input, counts);
    paintGlow(input, counts);
    paintNodes(input, counts);
  }
  paintLabels(input, counts);
  return counts;
}
