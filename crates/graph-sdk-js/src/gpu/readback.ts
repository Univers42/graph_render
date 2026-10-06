/**
 * `readback.ts` — the three read-back columns, the repeat fault's write, and the report.
 *
 * It is a separate file from `charge.ts` because the two halves of the pass have different
 * owners: `charge.ts` decides *what runs*, and this file decides *what the bytes mean*. The
 * read-back is the only place the device's output becomes a number, and the report is the only
 * place a number becomes a verdict, so both are here and neither is in the dispatch.
 *
 * ## The three columns, and why each is read back
 *
 * | column | for | how |
 * |---|---|---|
 * | `density` | the exact total **and** the byte comparison | `Int32Array`, summed in `f64` |
 * | `delta` | the comparator | `Float32Array`, `2n` components |
 * | `extent` | `boundsExact` | `Float32Array`, four words |
 *
 * The density is read back **twice** — once per run — because `repeatEqual` is a byte comparison
 * of two runs' buffers and not a comparison of two numbers. A 1-quantum difference in one cell
 * is invisible to any comparison of a rounded number and is the entire point of the
 * `--break repeat` fault.
 *
 * **One `copyBufferToBuffer` per column, then `mapAsync`.** The copy is into a `MAP_READ`
 * buffer that is nothing else, which is the rule that keeps a mapped buffer from also being a
 * kernel's output; the mapping is awaited before the bytes are read, because reading a mapped
 * range before `mapAsync` resolves is a validation error and not a stale value.
 */

import type { Buffers } from "./buffers.ts";
import type { Fixture } from "./fixture.ts";
import { compare, components, maxAbsGuard, verdict } from "./bounds.ts";
import type { ChargeReport } from "./charge.ts";
import type { GPUDevice } from "./types.ts";
import { GPUMapMode } from "./types.ts";

/** One full run's read-back. */
export interface Ran {
  readonly density: Int32Array;
  readonly delta: Float32Array;
  readonly extent: Float32Array;
}

/** Maps the three read-back buffers and copies their bytes out. */
export async function readback(buffers: Buffers, planeBytes: number, nodeBytes: number): Promise<Ran> {
  await Promise.all([
    buffers.read.density.mapAsync(GPUMapMode.READ),
    buffers.read.delta.mapAsync(GPUMapMode.READ),
    buffers.read.extent.mapAsync(GPUMapMode.READ),
  ]);
  const density = new Int32Array(buffers.read.density.getMappedRange().slice(0));
  const delta = new Float32Array(buffers.read.delta.getMappedRange().slice(0, nodeBytes));
  const extent = new Float32Array(buffers.read.extent.getMappedRange().slice(0));
  buffers.read.density.unmap();
  buffers.read.delta.unmap();
  buffers.read.extent.unmap();
  return { density, delta, extent };
}

/**
 * The `--break repeat` fault: `+1` into one density cell, after the first run's deposit and
 * before the second run's forward transform.
 *
 * It is a host-side write rather than a kernel because it must land between two runs of the
 * whole pass; a uniform could not, since the second run overwrites it.
 */
export function bumpDensity(device: GPUDevice, buffers: Buffers): void {
  device.queue.writeBuffer(buffers.density, 0, new Int32Array([1]));
}

/** Whether the two runs' bytes are equal, and the comparator's numbers against them. */
export function report(
  fixture: Fixture,
  first: Ran,
  second: Ran,
  marks: string,
  fallback: boolean,
): ChargeReport {
  const reference = components(fixture.delta.charge.x, fixture.delta.charge.y);
  const measured = compare({ got: second.delta, reference });
  const deposited = depositedUnits(second.density);
  const densityEqual = sameBytes(first.density, second.density);
  const deltaEqual = sameBytes(first.delta, second.delta);
  const repeatEqual = densityEqual && deltaEqual;
  const boundsExact = extentMatches(first.extent, fixture);
  const verdicted = verdict({
    n: fixture.n,
    state: fixture.state,
    arm: "hardware",
    h: fixture.h,
    rmsRel: measured.rmsRel,
    maxAbs: measured.maxAbs,
    depositedUnits: deposited,
    repeatEqual,
    repeatDetail: repeatDetailFor(densityEqual, deltaEqual, first.delta, second.delta),
    boundsExact,
  });
  return {
    n: fixture.n,
    state: fixture.state,
    side: fixture.side,
    rmsAbs: measured.rmsAbs,
    rmsRef: measured.rmsRef,
    rmsRel: measured.rmsRel,
    maxAbs: measured.maxAbs,
    depositedUnits: deposited,
    repeatEqual,
    boundsExact,
    pass: verdicted.pass,
    failures: verdicted.failures,
    marks,
    fallback,
    maxAbsGuard: fixture.n === 1_000_000 ? maxAbsGuard(fixture.h) : 0,
  };
}

/** `Σ density` over the whole buffer, in `f64` so the total is exact at 1M. */
function depositedUnits(density: Int32Array): number {
  let total = 0;
  for (let k = 0; k < density.length; k += 1) {
    total += density[k] ?? 0;
  }
  return total;
}

/** Byte equality of two typed arrays, not numeric equality — the point of the repeat check. */
function sameBytes(left: Int32Array | Float32Array, right: Int32Array | Float32Array): boolean {
  if (left.length !== right.length) {
    return false;
  }
  const a = new Uint8Array(left.buffer, left.byteOffset, left.byteLength);
  const b = new Uint8Array(right.buffer, right.byteOffset, right.byteLength);
  for (let k = 0; k < a.length; k += 1) {
    if (a[k] !== b[k]) {
      return false;
    }
  }
  return true;
}

/**
 * Which column differed, for the failure's message.
 *
 * Named rather than left as "density or delta": a repeat failure is a non-determinism, and a
 * non-determinism in the density (the deposit's atomics) and one in the delta (the transform) are
 * different faults with different fixes. The message says which, so the fix starts in the right
 * file.
 */
function repeatDetailFor(
  densityEqual: boolean,
  deltaEqual: boolean,
  first: Float32Array,
  second: Float32Array,
): string {
  if (!densityEqual) {
    return "the second run's density differs from the first";
  }
  return firstDeltaDifference(first, second);
}

/**
 * The first index at which two runs' delta columns differ, and the two values there.
 *
 * A non-determinism is a statement about *where* as much as *whether*: the index says which
 * node, and the two values say whether the difference is one rounding step (a reassociation the
 * compiler chose differently per run — not possible here, but the number that would show it) or
 * an O(1) one (a race). Without it, a repeat failure is a yes/no with no first place to look.
 */
function firstDeltaDifference(
  left: Float32Array,
  right: Float32Array,
): string {
  const count = Math.min(left.length, right.length);
  for (let k = 0; k < count; k += 1) {
    const a = left[k] ?? 0;
    const b = right[k] ?? 0;
    if (a !== b) {
      return `the second run's delta differs from the first at component ${k} (${a} then ${b})`;
    }
  }
  return "the second run's delta differs from the first";
}

/**
 * `boundsExact`: the device's `f32` min/max equal the host's, bit for bit.
 *
 * The host's are taken over `Math.fround` of the same fixture positions, so this measures the
 * *narrowing and the fold*, not whether the CPU's `f64` bounds would survive `f32` — that is a
 * different question and one this tier does not answer. The comparison is on `Uint32` views of
 * the two `Float32Array`s, so `-0.0` against `+0.0` is a difference, which is what "bit for bit"
 * has to mean here.
 */
function extentMatches(got: Float32Array, fixture: Fixture): boolean {
  const host = hostExtent(fixture);
  const mine = new Uint32Array(got.buffer, got.byteOffset, 4);
  const theirs = new Uint32Array(host.buffer, host.byteOffset, 4);
  for (let k = 0; k < 4; k += 1) {
    if (mine[k] !== theirs[k]) {
      return false;
    }
  }
  return true;
}

/** `(min x, min y, max x, max y)` over `Math.fround` of the fixture's positions. */
function hostExtent(fixture: Fixture): Float32Array {
  let loX = Number.POSITIVE_INFINITY;
  let loY = Number.POSITIVE_INFINITY;
  let hiX = Number.NEGATIVE_INFINITY;
  let hiY = Number.NEGATIVE_INFINITY;
  for (let k = 0; k < fixture.n; k += 1) {
    const x = Math.fround(fixture.posX[k] ?? 0);
    const y = Math.fround(fixture.posY[k] ?? 0);
    if (x < loX) loX = x;
    if (y < loY) loY = y;
    if (x > hiX) hiX = x;
    if (y > hiY) hiY = y;
  }
  return Float32Array.from([loX, loY, hiX, hiY]);
}