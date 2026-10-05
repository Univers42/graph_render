/** The ground under every frame, 2D or 3D: the theme's background, or the backdrop that replaces it. */
import type { PaintInput } from "./input.ts";

/**
 * Painted inside the frame the view already draws: no timer, no frame request. It resets the
 * transform and the alpha, so whatever the last frame left set does not tint the next ground.
 * Ponytail: the aurora is one diagonal three-stop gradient, not an animated field; it looks
 * flat on a very small viewport, and its corners are the first and last stop, not blends.
 */
export function paintGround(input: PaintInput): void {
  const { ctx, dpr, theme, viewport } = input;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.globalAlpha = 1;
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
