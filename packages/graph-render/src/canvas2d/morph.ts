/**
 * A routed edge mid-transition, bent part of the way instead of drawn straight until the nodes
 * arrive. The route is re-expressed in its own chord's frame (u along the chord, v across it)
 * and rebuilt on the chord the edge has now, with v scaled by how far the move has gone: the
 * old route straightens over the first half while the new one grows out of the straight line
 * over the second. The camera is a similarity, so u and v are the same in world and on screen,
 * and a curve's control points stay its control points under the map.
 *
 * Ponytail: the old route is relative to the old frame's own positions, so a move cut short by
 * another one snaps from the shape on screen to the old route's own; an edge set that differs
 * between the two frames (another document) grows the new route alone; and a Line frame under
 * the curve style trades its AUTO bend for the route in one frame. Each is one frame of snap,
 * never a wrong final drawing: the settled frame traces the frame's own route.
 */
import type { Frame } from "../frame.ts";
import type { PaintInput } from "./input.ts";
import { type Ends, type Route, frameRoute, newRoute, routeX, routeY, traceRoute } from "./route.ts";

export interface EdgeTween {
  /** The frame the nodes left, when it is routed and its edges are the new frame's own, else null. */
  readonly from: Frame | null;
  /** The eased fraction of the move: 0 at the start, 1 on arrival. */
  readonly eased: number;
}

/** True when `a` and `b` join the same pairs in the same order, so an old route belongs to the new edge. */
export function sameEdges(a: Frame, b: Frame): boolean {
  if (a.edgeCount !== b.edgeCount) return false;
  for (let edge = 0; edge < a.edgeCount; edge += 1) {
    if (a.source[edge] !== b.source[edge] || a.target[edge] !== b.target[edge]) return false;
  }
  return true;
}

/** The old frame worth morphing from: routed, with the new frame's edges. O(E), once per move. */
export function morphSource(old: Frame, next: Frame): Frame | null {
  return old.edgeKind !== "Line" && sameEdges(old, next) ? old : null;
}

/** Below this squared length, in CSS pixels, a chord has no direction to measure a route against. */
const MIN_CHORD2 = 1e-6;

/** Which route this frame draws and how bent: one leg per frame, reused so no edge allocates. */
const leg: { frame: Frame | null; bend: number } = { frame: null, bend: 0 };
const own = newRoute();
const moved = newRoute();
const chord: Ends = { ax: 0, ay: 0, bx: 0, by: 0 };
let shaped: Float32Array = new Float32Array(0);

function pickLeg(input: PaintInput, tween: EdgeTween): void {
  const eased = tween.eased;
  const from = tween.from !== null && tween.from.edgeKind !== "Line" ? tween.from : null;
  const to = input.frame.edgeKind === "Line" ? null : input.frame;
  const both = from !== null && to !== null;
  const old = from !== null && (to === null || eased < 0.5);
  leg.frame = old ? from : to;
  if (old) leg.bend = both ? 1 - 2 * eased : 1 - eased;
  else leg.bend = both ? 2 * eased - 1 : eased;
}

/** The edge's chord in the leg's own frame, on screen by the same map as its route. */
function chordOf(frame: Frame, edge: number, route: Route): Ends {
  const source = frame.source[edge] ?? 0;
  const target = frame.target[edge] ?? 0;
  chord.ax = (frame.x[source] ?? 0) * route.scale + route.dx;
  chord.ay = (frame.y[source] ?? 0) * route.scale + route.dy;
  chord.bx = (frame.x[target] ?? 0) * route.scale + route.dx;
  chord.by = (frame.y[target] ?? 0) * route.scale + route.dy;
  return chord;
}

/** Moves the route's points from `base` onto `ends`, `bend` of the way bent; false for a degenerate base. */
function reshape(route: Route, base: Ends, ends: Ends, bend: number): boolean {
  const cx = base.bx - base.ax;
  const cy = base.by - base.ay;
  const length2 = cx * cx + cy * cy;
  if (!(length2 > MIN_CHORD2)) return false;
  const dx = ends.bx - ends.ax;
  const dy = ends.by - ends.ay;
  const count = route.to - route.from;
  if (shaped.length < 2 * count) shaped = new Float32Array(2 * count);
  for (let at = 0; at < count; at += 1) {
    const wx = routeX(route, route.from + at) - base.ax;
    const wy = routeY(route, route.from + at) - base.ay;
    const u = (wx * cx + wy * cy) / length2;
    const v = (cx * wy - cy * wx) / length2;
    // Straight means on the segment: a point past an end is pulled back onto it as the bend goes.
    const inside = Math.min(1, Math.max(0, u));
    const along = inside + bend * (u - inside);
    shaped[2 * at] = ends.ax + along * dx - bend * v * dy;
    shaped[2 * at + 1] = ends.ay + along * dy + bend * v * dx;
  }
  Object.assign(moved, { pts: shaped, from: 0, to: count, kind: route.kind, degree: route.degree });
  return true;
}

/** Traces `edge` from `ends`, which the moveTo already started, while the nodes are on their way. */
export function traceMorph(input: PaintInput, edge: number, ends: Ends): void {
  const tween = input.tween;
  if (tween === undefined || tween === null) {
    input.ctx.lineTo(ends.bx, ends.by);
    return;
  }
  pickLeg(input, tween);
  const route = leg.frame === null ? null : frameRoute(leg.frame, edge, input.camera, own);
  if (route === null || leg.frame === null || !reshape(route, chordOf(leg.frame, edge, route), ends, leg.bend)) {
    input.ctx.lineTo(ends.bx, ends.by);
    return;
  }
  traceRoute(input.ctx, moved, ends);
}
