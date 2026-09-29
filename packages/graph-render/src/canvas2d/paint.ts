/** One frame on a 2D context: background, edges, nodes, labels — in that order. */
import { type PaintCounts, type PaintInput, newCounts } from "./input.ts";
import { paintEdges } from "./edges.ts";
import { paintGlow } from "./glow.ts";
import { paintNodes } from "./nodes.ts";

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

export function paintFrame(input: PaintInput): PaintCounts {
  const counts = newCounts();
  const { ctx, dpr, viewport } = input;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalAlpha = 1;
  ctx.fillStyle = input.theme.background;
  ctx.fillRect(0, 0, viewport.width, viewport.height);
  paintEdges(input, counts);
  paintGlow(input, counts);
  paintNodes(input, counts);
  paintLabels(input, counts);
  return counts;
}
