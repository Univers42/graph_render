/**
 * A routed edge's interior traced onto the canvas: the curve its points are the control points
 * of, or the polyline through them. Out of edges.ts so the transition morph (morph.ts) traces
 * the points it computes exactly the way a settled frame traces the frame's own.
 */
import { bezierAt, FLAT_SEGMENTS, type Sample } from "../edges2d/curve.ts";
import type { Camera } from "../camera.ts";
import type { Frame } from "../frame.ts";
import type { EdgeKind } from "../snapshot/decode.ts";
import type { Surface2D } from "./surface.ts";

/** Screen ends of one edge. */
export interface Ends {
  ax: number;
  ay: number;
  bx: number;
  by: number;
}

/** One edge's interior points, `pts[2 * from]` up to `pts[2 * to]`, put on screen by `scale` then `dx`/`dy`. */
export interface Route {
  pts: Float32Array;
  from: number;
  to: number;
  scale: number;
  dx: number;
  dy: number;
  kind: EdgeKind;
  degree: number;
}

export function newRoute(): Route {
  return { pts: new Float32Array(0), from: 0, to: 0, scale: 1, dx: 0, dy: 0, kind: "Line", degree: 0 };
}

/** The edge's route in `frame`, put on screen by `camera`, or null for a Line frame, which carries none. */
export function frameRoute(frame: Frame, edge: number, camera: Camera, out: Route): Route | null {
  if (frame.edgeKind === "Line" || frame.pts === null) return null;
  out.pts = frame.pts;
  out.from = frame.offsets?.[edge] ?? 0;
  out.to = frame.offsets?.[edge + 1] ?? 0;
  out.scale = camera.scale;
  out.dx = camera.x;
  out.dy = camera.y;
  out.kind = frame.edgeKind;
  out.degree = frame.curveDegree;
  return out;
}

export function routeX(route: Route, point: number): number {
  return (route.pts[2 * point] ?? 0) * route.scale + route.dx;
}

export function routeY(route: Route, point: number): number {
  return (route.pts[2 * point + 1] ?? 0) * route.scale + route.dy;
}

/** The control polygon in screen coordinates, grown and never shrunk, and the point de Casteljau last left it at. */
let polygon: Float32Array = new Float32Array(0);
const sampled: Sample = { x: 0, y: 0 };

/** Loads the edge's own control polygon, its two endpoints included, in dense order. */
function loadPolygon(route: Route, ends: Ends): number {
  const count = route.to - route.from + 2;
  if (polygon.length < 2 * count) polygon = new Float32Array(2 * count);
  polygon[0] = ends.ax;
  polygon[1] = ends.ay;
  for (let at = 0; at < route.to - route.from; at += 1) {
    polygon[2 * (at + 1)] = routeX(route, route.from + at);
    polygon[2 * (at + 1) + 1] = routeY(route, route.from + at);
  }
  polygon[2 * (count - 1)] = ends.bx;
  polygon[2 * (count - 1) + 1] = ends.by;
  return count;
}

/** Degree 4 and up: the curve itself, one chord per FLAT_SEGMENTS, never the polygon. */
function traceHigher(ctx: Surface2D, route: Route, ends: Ends): void {
  const count = loadPolygon(route, ends);
  for (let step = 1; step <= FLAT_SEGMENTS; step += 1) {
    bezierAt(polygon, count, step / FLAT_SEGMENTS, sampled);
    ctx.lineTo(sampled.x, sampled.y);
  }
}

/**
 * Interior points as the control points of the curve when the count is the degree's, and as
 * a polyline through themselves when it is not. Degree 2 and 3 go to the exact canvas calls;
 * a higher degree is flattened by de Casteljau (edges2d/curve.ts). The points a snapshot
 * carries are the geometry it has: a count that disagrees with the degree cannot be evaluated
 * without inventing control points the writer never stored, so those edges are drawn as the
 * polyline their own points describe rather than as a curve of the declared degree.
 *
 * Ponytail: a count that disagrees with the degree — a malformed Curve, which a writer
 * emitting both never does — is drawn as its control polygon, so the bend reads as kinks at
 * the stored points; a degree of 1 carries no interior point and is the straight chord, which
 * the polyline already draws.
 */
export function traceRoute(ctx: Surface2D, route: Route, ends: Ends): void {
  const { from, to, degree } = route;
  if (route.kind === "Curve" && degree >= 2 && to - from === degree - 1) {
    if (degree === 2) ctx.quadraticCurveTo(routeX(route, from), routeY(route, from), ends.bx, ends.by);
    else if (degree === 3) ctx.bezierCurveTo(routeX(route, from), routeY(route, from), routeX(route, from + 1), routeY(route, from + 1), ends.bx, ends.by);
    else traceHigher(ctx, route, ends);
    return;
  }
  for (let p = from; p < to; p += 1) ctx.lineTo(routeX(route, p), routeY(route, p));
  ctx.lineTo(ends.bx, ends.by);
}
