/**
 * One 3D frame, projected once: every node's screen point, its depth, its drawn radius, and
 * the order the painter draws them in. The painter reads this and never projects again, so
 * a frame costs one projection and one sort however many times it draws.
 *
 * It is a value the view owns and reuses across frames, like the label plan beside it: the
 * columns are read into it every frame and never kept, so the next frame replaces them.
 *
 * Ponytail: there is no z-buffer and no per-node shading, so a node's depth buys it order
 * and a wider drawn radius and nothing else — `three/orbit.ts` carries the rest of the
 * caveats. Edge paths stay 2D in the contract, so a 3D edge is the straight line between
 * its two ends' projected points.
 */
import type { Viewport } from "../camera.ts";
import type { Frame } from "../frame.ts";
import {
  type Basis, type Orbit, type Projected as At, type Vec3, basisOf, focalOf, project, radiusOnScreen,
} from "./orbit.ts";
import { depthOrder } from "./sort.ts";

/**
 * One node as drawn, in the order it is painted: where on the screen, and how far from the
 * eye. What a host reads to say what is on screen without asking the painter.
 */
export interface Projected {
  /** Dense index of the node, so the order is not mistaken for the node order. */
  readonly node: number;
  readonly x: number;
  readonly y: number;
  /** Eye to node, in world units. Always above zero: a node behind the eye is not listed. */
  readonly depth: number;
}

/** One node as drawn: where, how near, and how wide its world radius came out. */
export interface Drawn {
  readonly x: Float32Array;
  readonly y: Float32Array;
  /**
   * Eye to node, in world units. 0 for a node behind the eye, which is not drawn and is not
   * in the order. A f64, not an f32: the sort quantises this into buckets, and a 32-bit
   * depth would put a 4000-unit-deep drawing's nodes into buckets a hundredth of a unit
   * wide, which is a lot of integers to buy nothing.
   */
  readonly depth: Float64Array;
  /** The drawn radius in CSS pixels: the node's world radius over its depth. */
  readonly radius: Float32Array;
  /** Node indices, furthest first. The painter walks this and nothing else. */
  order: Uint32Array;
  /** How many entries of `order` are real; a node behind the eye is not one of them. */
  drawn: number;
}

/** What one projection reads. The columns are the ones being drawn, not the frame's own. */
export interface Projection {
  readonly frame: Frame;
  readonly x: Float32Array;
  readonly y: Float32Array;
  /** World radius per node, as the 2D painter's `extent`. */
  readonly extent: Float32Array;
  readonly orbit: Orbit;
  readonly viewport: Viewport;
}

export function newProjection(count: number): Drawn {
  return {
    x: new Float32Array(count),
    y: new Float32Array(count),
    depth: new Float64Array(count),
    radius: new Float32Array(count),
    order: new Uint32Array(count),
    drawn: 0,
  };
}

function centreOf(viewport: Viewport): Vec3 {
  return { x: viewport.width / 2, y: viewport.height / 2, z: 0 };
}

/** The basis and the two numbers a projection needs, so a pass takes one argument. */
interface Setup {
  readonly basis: Basis;
  readonly focal: number;
  readonly centre: Vec3;
}

function setupOf(wanted: Projection): Setup {
  return {
    basis: basisOf(wanted.orbit),
    focal: focalOf(wanted.orbit, wanted.viewport),
    centre: centreOf(wanted.viewport),
  };
}

/** The node's own numbers, as one argument: a store that takes five is a store to split. */
interface Placed {
  readonly node: number;
  readonly found: At;
  readonly worldRadius: number;
}

function store(out: Drawn, placed: Placed, setup: Setup): void {
  const { node, found, worldRadius } = placed;
  out.x[node] = found.x;
  out.y[node] = found.y;
  out.depth[node] = found.depth;
  out.radius[node] = radiusOnScreen(setup.focal, worldRadius, found.depth);
}

function blank(out: Drawn, node: number): void {
  out.x[node] = 0;
  out.y[node] = 0;
  out.depth[node] = 0;
  out.radius[node] = 0;
}

/** One pass over the nodes: the screen point, the depth and the drawn radius of each. */
function projectAll(out: Drawn, wanted: Projection, setup: Setup): void {
  const { frame, x, y, extent, z } = { ...wanted, z: wanted.frame.z };
  for (let node = 0; node < frame.nodeCount; node += 1) {
    const point = { x: x[node] ?? 0, y: y[node] ?? 0, z: z?.[node] ?? 0 };
    const found = project(setup.basis, point, setup.focal, setup.centre);
    if (found === null) blank(out, node);
    else store(out, { node, found, worldRadius: extent[node] ?? 0 }, setup);
  }
}

/**
 * Projects every node of `wanted` into `into` and leaves the paint order there. The order is
 * by depth, so a node behind the eye (a depth of 0) is not in it and the painter draws no
 * trace of it.
 */
export function projectFrame(into: Drawn, wanted: Projection): Drawn {
  const out = into.x.length === wanted.frame.nodeCount ? into : newProjection(wanted.frame.nodeCount);
  projectAll(out, wanted, setupOf(wanted));
  out.order = depthOrder(out.depth, out.order);
  out.drawn = out.order.length;
  return out;
}
