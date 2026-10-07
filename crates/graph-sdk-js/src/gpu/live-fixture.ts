/**
 * `live-fixture.ts` — a session's graph as the `Fixture` a rig is built over.
 *
 * Every stage takes a `Fixture` (`fixture.ts`), and a `.gmfx` is one session's state written
 * to a file. The live arm builds the same object from the session itself: its `f64`
 * positions, its simple graph (`force-handoff.ts`) and its parameters (`law.ts`), so the
 * stages keep one input type and the probe and the live arm share every dispatch.
 *
 * What the file would have carried and the session does not, derived here:
 *
 * - **the side**, `mesh.rs:31-34`'s `side_for`;
 * - **the twiddles**, `fft.rs:81-112`'s forward table;
 * - **the frame**, `frame.ts`'s `frameOf` over the `f64` positions at the law's `distance_max`,
 *   the fold graph-core places its own first tick with;
 * - **no spectrum**: an empty one, which makes the charge stage refresh its kernel at the
 *   first placement (`charge.ts`) instead of trusting a spectrum nobody computed;
 * - **no deltas**: those are the probe's expected answers, and a live tick has none.
 *
 * Caveat: the twiddles are `Math.cos`/`Math.sin`, which the language leaves
 * implementation-approximated, where graph-core calls `libm`; the two can differ in the last
 * `f64` bit, and the upload narrows both to `f32`, 29 bits coarser, so the device sees the same
 * twiddle except where an `f64` ulp straddles an `f32` rounding boundary.
 */

import type { ForceParams } from "../types.ts";
import type { Fixture } from "./fixture.ts";
import { frameOf } from "./frame.ts";
import type { Frame } from "./frame.ts";

/** What the live arm reads off a session to build its rig. */
export interface LiveGraph {
  readonly posX: Float64Array;
  readonly posY: Float64Array;
  readonly lo: Uint32Array;
  readonly hi: Uint32Array;
  readonly strength: Float64Array;
}

/** `mesh.rs`'s `MAX_SIDE`. */
const MAX_SIDE = 1024;

/** `side_for` (`mesh.rs:31-34`): `ceil(√n)`, up to a power of two, clamped to `128..=1024`. */
export function sideFor(n: number): number {
  const root = Math.ceil(Math.sqrt(n));
  const power = root <= 1 ? 1 : 2 ** Math.ceil(Math.log2(root));
  return Math.min(Math.max(power, 128), MAX_SIDE);
}

/** `Plan::new`'s forward table (`fft.rs:91-104`), split into its two columns. */
export function twiddlesFor(side: number): { re: Float64Array; im: Float64Array } {
  const re = new Float64Array(side);
  const im = new Float64Array(side);
  for (let k = 0; k < side; k += 1) {
    const half = 2 ** Math.floor(Math.log2(Math.max(k, 1)));
    const root = (k % half) * (side / (2 * half));
    const angle = (-2 * Math.PI * root) / side;
    re[k] = Math.cos(angle);
    im[k] = Math.sin(angle);
  }
  return { re, im };
}

/** The rig input for one session's state at one law. */
export function liveFixture(graph: LiveGraph, law: Readonly<ForceParams>): Fixture {
  const n = graph.posX.length;
  const side = sideFor(n);
  const frame = frameOf(graph.posX, graph.posY, side, law.distance_max) ?? EMPTY_FRAME;
  const twiddles = twiddlesFor(side);
  const none = new Float64Array(0);
  const delta = { x: none, y: none };
  return {
    n,
    m: graph.lo.length,
    side,
    state: 0,
    ...frame,
    edgeLo: graph.lo,
    edgeHi: graph.hi,
    edgeStrength: graph.strength,
    posX: graph.posX,
    posY: graph.posY,
    twiddleRe: twiddles.re,
    twiddleIm: twiddles.im,
    spectrumRe: none,
    spectrumIm: none,
    delta: { link: delta, charge: delta, collide: delta },
    law,
  };
}

/**
 * The frame of a graph with no finite position, where `frameOf` answers `null` and the CPU's
 * charge returns before it reads a field (`mesh.rs:164-170`). The tick re-derives the frame
 * from the read-back after the first tick, so this only places the rig's first deposit.
 */
const EMPTY_FRAME: Frame = { step: 0, h: 1, originX: 0, originY: 0, cells: 3, reach: 2 };
