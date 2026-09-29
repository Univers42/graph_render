/**
 * The quadratic control point of a Line edge: SciGraphs' AUTO bend, so the studio's
 * Line edges curve exactly the way SciGraphs' curved edges do.
 *   core/scigraphs_core/mesh/edge_styles.py:108-145 (perpendicular, AUTO sign)
 *   core/scigraphs_core/mesh/edge_styles.py:162-188 (offset, midpoint)
 *
 * `from` and `to` are in SciGraphs' world frame, which is y-up: text_overlay.py:231-233
 * flips y only on the way into pixels. A caller holding y-down screen coordinates passes
 * (x, -y) and negates the y of the control point it gets back, because the formula is not
 * invariant under that flip and would otherwise bend every edge with |dx| > |dy| the
 * other way.
 */
import type { Point } from "../camera.ts";

/** Below this the edge has no direction to bend in (edge_styles.py:117). */
const MIN_LENGTH = 1e-10;

/** Below this curvature the source returns the straight edge (edge_styles.py:170). */
const MIN_CURVATURE = 0.001;

/**
 * offset = L * curvature * 0.5 (edge_styles.py:174), and 0.5 is what CYTOSCAPE_BEZIER
 * carries as its own edge_curvature (edge_styles.py:21-28), so it is the default here.
 */
function offsetOf(length: number, curvature: number): number {
  return length * curvature * 0.5;
}

/**
 * The unit perpendicular in the XY plane, normalize(dy, -dx): the source takes cross(d, up)
 * with up = (0,0,1), which is exactly (dy, -dx) (edge_styles.py:122-124). It comes out unit
 * because the caller has already divided d by its length, so the source's 3D fallback at
 * :127-134, which only serves an edge parallel to z, has nothing left to catch here.
 */
function perpendicular(dx: number, dy: number, length: number): Point {
  return { x: dy / length, y: -dx / length };
}

/**
 * AUTO: the sign comes from the coordinates and not from the direction of travel, so an
 * edge bends the same way whichever endpoint is named (edge_styles.py:113,143). On a tie of
 * x0+y0 == x1+y1 the comparison is not strict and the sign is -1 either way, so the two
 * directions do disagree there, in the source too.
 */
function autoSign(from: Point, to: Point): number {
  return from.x + from.y > to.x + to.y ? 1 : -1;
}

/**
 * The quadratic control point for a curved edge, or null when there is no bend to apply:
 * a length under 1e-10 (edge_styles.py:117, where the source instead gets a zero offset and
 * so the bare midpoint) or curvature below 0.001 (:170). Endpoints that close are a
 * self-loop in the source (edge_styles.py:458-459) and drawing one is the caller's business.
 */
export function controlPoint(from: Point, to: Point, curvature = 0.5): Point | null {
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.hypot(dx, dy);
  if (length < MIN_LENGTH || curvature < MIN_CURVATURE) return null;
  const perp = perpendicular(dx, dy, length);
  const offset = offsetOf(length, curvature) * autoSign(from, to);
  return {
    x: (from.x + to.x) / 2 + perp.x * offset,
    y: (from.y + to.y) / 2 + perp.y * offset,
  };
}
