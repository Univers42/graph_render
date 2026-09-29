/**
 * The glow: two translucent discs behind every node, batched per colour like the nodes
 * themselves. The halo is HALO_REACH radii wide per unit of strength, so it grows with
 * the setting and with the node.
 *
 * Ponytail: a stack of flat discs, not a gaussian blur (the 2D surface the painter is
 * handed has no shadow or filter). Two layers band visibly on a large node against a
 * dark ground; and overlapping halos add their alpha, so a dense cluster glows brighter
 * than a lone node. Halos are not drawn for Box nodes, whose glow would be a rectangle.
 */
import type { PaintCounts, PaintInput } from "./input.ts";
import { MIN_SCREEN_RADIUS } from "./nodes.ts";

const HALO_REACH = 0.9;
const LAYERS = [{ grow: 1, alpha: 0.1 }, { grow: 0.5, alpha: 0.16 }] as const;
const TAU = Math.PI * 2;

function traceHalos(input: PaintInput, bucket: number, grow: number): number {
  const { ctx, camera, viewport, style } = input;
  const end = style.bucketStart[bucket + 1] ?? 0;
  let drawn = 0;
  for (let at = style.bucketStart[bucket] ?? 0; at < end; at += 1) {
    const node = style.bucketItems[at] ?? 0;
    if (style.hidden?.[node] === 1) continue;
    const radius = Math.max(MIN_SCREEN_RADIUS, (input.extent[node] ?? 0) * camera.scale);
    const reach = radius * (1 + HALO_REACH * style.glow * grow);
    const sx = (input.x[node] ?? 0) * camera.scale + camera.x;
    const sy = (input.y[node] ?? 0) * camera.scale + camera.y;
    if (sx < -reach || sy < -reach || sx > viewport.width + reach || sy > viewport.height + reach) continue;
    ctx.moveTo(sx + reach, sy);
    ctx.arc(sx, sy, reach, 0, TAU);
    drawn += 1;
  }
  return drawn;
}

export function paintGlow(input: PaintInput, counts: PaintCounts): void {
  const { ctx, style } = input;
  if (style.glow <= 0 || input.frame.nodeKind === "Box") return;
  for (const layer of LAYERS) {
    ctx.globalAlpha = layer.alpha * (input.focus >= 0 ? input.theme.dimAlpha : 1);
    for (let bucket = 0; bucket < style.palette.length; bucket += 1) {
      ctx.beginPath();
      if (traceHalos(input, bucket, layer.grow) === 0) continue;
      ctx.fillStyle = style.palette[bucket] ?? "#9a9a9a";
      ctx.fill();
      counts.draws += 1;
      counts.glowFills += 1;
    }
  }
  ctx.globalAlpha = 1;
}
