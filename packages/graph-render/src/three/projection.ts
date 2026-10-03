/**
 * One 3D frame, projected at most once and only when read: every node's screen point, its
 * depth, its drawn radius, and the order the painter draws them in. The Canvas2D painter
 * reads this and never projects again, so a frame it paints costs one projection and one sort
 * however many times it draws.
 *
 * It is a value the view owns and reuses across frames, like the label plan beside it. The
 * loop hands it the frame's columns every frame (`projectFrame`, O(1)); the first read of a
 * field after that projects them, and the next frame's hand-over makes them stale again. A
 * frame the WebGL2 3D path draws (`webgl2/hook3d.ts`) reads none of the columns, only
 * `wanted`, so an orbit drag on the GPU costs no projection and no sort at all.
 *
 * Caveat: the read is late, not at hand-over. A hover pick between two frames projects the
 * columns as they are at the pick, so a node the view moved in place since the last paint is
 * picked where it now is rather than where it was drawn; the loop repaints such a move on the
 * next frame, so the two agree again within one frame.
 *
 * Ponytail: the Canvas2D painter has no z-buffer and no per-node shading, so a node's depth
 * buys it order and a wider drawn radius and nothing else — `three/orbit.ts` carries the rest
 * of the caveats. A box's w/h and a routed edge's interior points are projected here too, and
 * `three/paths.ts` says what it had to invent a z for.
 */
import type { Viewport } from "../camera.ts";
import type { Frame } from "../frame.ts";
import {
  type Basis, type Orbit, type Projected as At, type Vec3, basisOf, focalOf, project, radiusOnScreen,
} from "./orbit.ts";
import { depthOrder } from "./sort.ts";
import { boxHalfExtents, interiorPoints, pixelsPerUnit } from "./paths.ts";

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
  /**
   * Two entries per node: the node's own projected half width and half height, in CSS
   * pixels, for a `Box` frame (`three/paths.ts`). Both zero when the frame carries no w/h
   * column or the node is behind the eye, so a caller can ask without asking the frame.
   */
  readonly boxHalf: Float32Array;
  /**
   * Every edge's interior points, projected: two entries per point, indexed by the point's
   * own index in `frame.pts`, so the painter reads the point it is tracing and never
   * projects. Replaced when a new frame carries a different number of points.
   */
  readonly points: Float32Array;
  /**
   * Pixels one world unit covers at the drawing's mean depth — the 3D painter's answer to
   * the 2D camera's `scale`, and the one number an edge stroke width can be taken over
   * (`three/paths.ts`). 0 when nothing is in front of the eye.
   */
  readonly ppu: number;
  /** Node indices, furthest first. The painter walks this and nothing else. */
  readonly order: Uint32Array;
  /** How many entries of `order` are real; a node behind the eye is not one of them. */
  readonly drawn: number;
  /** What the columns above are projected from, or null before the first frame. */
  readonly wanted: Projection | null;
}

/** The columns one projection writes, which `Drawn` hands out once they are current. */
interface Columns {
  x: Float32Array;
  y: Float32Array;
  depth: Float64Array;
  radius: Float32Array;
  boxHalf: Float32Array;
  points: Float32Array;
  ppu: number;
  order: Uint32Array;
  drawn: number;
}

function columnsOf(count: number): Columns {
  return {
    x: new Float32Array(count),
    y: new Float32Array(count),
    depth: new Float64Array(count),
    radius: new Float32Array(count),
    boxHalf: new Float32Array(count * 2),
    points: new Float32Array(0),
    ppu: 0,
    order: new Uint32Array(count),
    drawn: 0,
  };
}

function centreOf(viewport: Viewport): Vec3 {
  return { x: viewport.width / 2, y: viewport.height / 2, z: 0 };
}

/** The basis and the two numbers a projection needs, so a pass takes one argument. */
export interface Setup {
  readonly basis: Basis;
  readonly focal: number;
  readonly centre: Vec3;
}

export function setupOf(wanted: Pick<Projection, "orbit" | "viewport">): Setup {
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

function store(out: Columns, placed: Placed, setup: Setup): void {
  const { node, found, worldRadius } = placed;
  out.x[node] = found.x;
  out.y[node] = found.y;
  out.depth[node] = found.depth;
  out.radius[node] = radiusOnScreen(setup.focal, worldRadius, found.depth);
}

function blank(out: Columns, node: number): void {
  out.x[node] = 0;
  out.y[node] = 0;
  out.depth[node] = 0;
  out.radius[node] = 0;
}

/** One pass over the nodes: the screen point, the depth and the drawn radius of each. */
function projectAll(out: Columns, wanted: Projection, setup: Setup): void {
  const { frame, x, y, extent, z } = { ...wanted, z: wanted.frame.z };
  for (let node = 0; node < frame.nodeCount; node += 1) {
    const point = { x: x[node] ?? 0, y: y[node] ?? 0, z: z?.[node] ?? 0 };
    const found = project(setup.basis, point, setup.focal, setup.centre);
    if (found === null) blank(out, node);
    else store(out, { node, found, worldRadius: extent[node] ?? 0 }, setup);
  }
}

/**
 * The shapes the disc and the chord do not cover: a node's own w/h, and every edge's
 * interior points. `three/paths.ts` holds the projection and the caveats; this only hands it
 * the setup and reads back what it left, because both need the nodes already placed.
 */
function projectShapes(out: Columns, wanted: Projection, setup: Setup): void {
  out.boxHalf = boxHalfExtents(out.boxHalf, wanted.frame, out.depth, setup.focal);
  out.points = interiorPoints(out.points, wanted, out, setup);
  out.ppu = pixelsPerUnit(out, setup.focal);
}

/**
 * Projects every node of `wanted` into `into` (or fresh columns when the node count moved)
 * and leaves the paint order there. The order is by depth, so a node behind the eye (a depth
 * of 0) is not in it and the painter draws no trace of it.
 */
function projectInto(into: Columns, wanted: Projection): Columns {
  const out = into.x.length === wanted.frame.nodeCount ? into : columnsOf(wanted.frame.nodeCount);
  const setup = setupOf(wanted);
  projectAll(out, wanted, setup);
  projectShapes(out, wanted, setup);
  out.order = depthOrder(out.depth, out.order);
  out.drawn = out.order.length;
  return out;
}

/** A `Drawn` whose columns are projected on their first read after each `aim`. */
class LateProjection implements Drawn {
  #columns: Columns;
  #wanted: Projection | null = null;
  #stale = false;

  constructor(count: number) {
    this.#columns = columnsOf(count);
  }

  aim(wanted: Projection): void {
    this.#wanted = wanted;
    this.#stale = true;
  }

  #ready(): Columns {
    if (this.#stale && this.#wanted !== null) this.#columns = projectInto(this.#columns, this.#wanted);
    this.#stale = false;
    return this.#columns;
  }

  get wanted(): Projection | null { return this.#wanted; }
  get x(): Float32Array { return this.#ready().x; }
  get y(): Float32Array { return this.#ready().y; }
  get depth(): Float64Array { return this.#ready().depth; }
  get radius(): Float32Array { return this.#ready().radius; }
  get boxHalf(): Float32Array { return this.#ready().boxHalf; }
  get points(): Float32Array { return this.#ready().points; }
  get ppu(): number { return this.#ready().ppu; }
  get order(): Uint32Array { return this.#ready().order; }
  get drawn(): number { return this.#ready().drawn; }
}

export function newProjection(count: number): Drawn {
  return new LateProjection(count);
}

/**
 * Hands `wanted` to `into` and returns it, projecting nothing: the first field read projects
 * the whole frame, once (see the file header). `into` is reused across frames.
 */
export function projectFrame(into: Drawn, wanted: Projection): Drawn {
  const out = into instanceof LateProjection ? into : new LateProjection(wanted.frame.nodeCount);
  out.aim(wanted);
  return out;
}
