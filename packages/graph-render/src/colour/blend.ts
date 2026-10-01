/**
 * Blending two linear-light colours along one edge, and the stops its gradient carries.
 *
 * Canvas2D interpolates a gradient in sRGB, not in linear light, so a two-stop gradient is
 * not the blend SciGraphs' Blender interpolates along an edge tube, and its midpoint comes
 * out darker than the mean of the two ends. The stops below are the linear-light mix at K
 * positions, each channel encoded exactly once with cssOf: the rasteriser then interpolates
 * in sRGB over a quarter of an edge, which is within one byte of the mean at every t.
 *   SciGraphs/ui/coloring/functions.py:298-330 (the ramp along a tube)
 *   SciGraphs/api/render.py:288-345 (linear values in, converted on output)
 */
import type { Rgb } from "./srgb.ts";
import { cssOf } from "./srgb.ts";

/** Stops per edge gradient. Three at least: two would ask the rasteriser to do the blend. */
export const GRADIENT_STOPS = 5;

export interface Stop {
  /** Where along the edge, 0 at the source and 1 at the target. */
  readonly offset: number;
  /** The CSS colour of the linear mix at `offset`, encoded once. */
  readonly colour: string;
}

/** The linear-light mix of `a` and `b` at `t`; a t outside 0..1 is clamped to it. */
export function mixLinear(a: Rgb, b: Rgb, t: number): Rgb {
  const f = t <= 0 ? 0 : t >= 1 ? 1 : t;
  return [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f, a[2] + (b[2] - a[2]) * f];
}

/** The linear mean of two colours: what a mixed edge is drawn in when it gives up. */
export function meanLinear(a: Rgb, b: Rgb): Rgb {
  return mixLinear(a, b, 0.5);
}

/**
 * The GRADIENT_STOPS stops one edge's gradient carries, endpoints exact. Odd, so the mean
 * is a stop of its own and lands on the byte the mix computes rather than between two.
 */
export function edgeStops(a: Rgb, b: Rgb): readonly Stop[] {
  return Array.from({ length: GRADIENT_STOPS }, (_, k) => {
    const offset = k / (GRADIENT_STOPS - 1);
    return { offset, colour: cssOf(mixLinear(a, b, offset)) };
  });
}