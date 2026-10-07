/**
 * `charge.ts` — the dispatch order, the uploads, the read-back, and the report.
 *
 * ## The dispatch order, which is `mesh.rs`'s
 *
 * `Mesh::solve` runs, in order: bounds, deposit, the kernel refresh, the forward transform,
 * the inverse transform (`mesh.rs:159-184`); then `charge::apply` reads the field per node
 * (`charge.rs:38-53`). The kernel refresh is **absent** — the fixture's `spectrum_*` is that
 * refresh's own output, already forward-transformed and pre-scaled by `1/P²`
 * (`fixtures/gpu/README.md:108-109`), so uploading it *is* the stage. That is what condition 6
 * rules on: the comparison is a transcription check, and the kernel spectrum is one of the
 * things it cannot see.
 *
 * So the dispatches, each in its own pass so the order is the encoder's and not a convention:
 *
 * ```
 * [zero_density · fold_block · fold_extent · deposit]  ← submit 1
 * [widen_density · fft_line × 4 · read_field]          ← submit 2
 * ```
 *
 * Two submits per run, split at the deposit, because the `--break repeat` fault writes `+1` into
 * one density cell *inside* the second run — after its deposit, before its transform — and a run
 * that is a single submit has no inside to write into. Written between the two runs instead, the
 * second run's own `zero_density` would erase it and the control would prove nothing.
 *
 * `live` per transform pass is the one `fft/pass.rs` gives and not another: `cells` for the
 * forward rows and the inverse columns, `side` for the other two (`fft/pass.rs:112-125`).
 *
 * ## The stage, which the tick reuses
 *
 * `buildChargeStage` builds the pipelines, binds the groups and uploads the constants once per
 * graph. The returned stage encodes its dispatches into any command encoder, so the probe and the
 * resident tick share one set of dispatches and one set of bindings. `encodeDeposit` takes a
 * `fold` flag: the probe folds the bounds on the device for its `boundsExact` check, the tick
 * does not — its frame comes from the host's read-back (`frame.ts`), so the device's fold would
 * be a second, redundant one.
 *
 * ## The whole pass runs twice
 *
 * `repeatEqual` is not a constant and not a comparison of two numbers: it is a byte comparison
 * of the `density` and `delta` buffers from two full runs in the same device and the same
 * process. The `--break repeat` fault writes `+1` into one density cell between the two runs'
 * deposits, so the comparison has to be on bytes — a 1-quantum difference would be rounded away
 * by any comparison of a rounded number.
 *
 * **One `copyBufferToBuffer` per read-back column, then `mapAsync`.** Three columns: the
 * density, for the exact total and for the byte comparison; the delta, for the comparator; the
 * extent, for `boundsExact`. Each is copied into its own `MAP_READ` buffer and the mapping is
 * awaited before the bytes are read — reading a mapped range before `mapAsync` resolves is a
 * validation error, not a stale value.
 */

import { Refusal, open } from "./adapter.ts";
import { create, destroy } from "./buffers.ts";
import type { Buffers } from "./buffers.ts";
import { refreshKernel } from "./charge-kernel.ts";
import { uploadConstants, writeFrames } from "./charge-upload.ts";
import { loadFixture } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import type { Arm } from "./bounds.ts";
import { bumpDensity, readback, report } from "./readback.ts";
import type { Ran } from "./readback.ts";
import { build, dispatch, groupsFor } from "./pipelines.ts";
import type { GPUBuffer, GPUCommandEncoder, GPUDevice, GpuHost } from "./types.ts";
import type { Frame } from "./frame.ts";

/** What the charge arm needs from the host. No fault knob: the harness passes it separately. */
export interface ChargeRequest {
  /** The `.gmfx` bytes, exactly as `emit-gpu-fixtures` wrote them. */
  readonly fixture: ArrayBuffer;
  /** Which arm to insist on. `"any"` takes what the device offers. */
  readonly arm?: Arm | "any";
  /**
   * The object carrying `gpu`. Omitted in a page — it then reads `navigator`, whose `gpu` is
   * declared in `types.ts` — and passed explicitly by a node test.
   */
  readonly host?: GpuHost;
}

/** One case's verdict. `pass` is false when any guard or ceiling is exceeded. */
export interface ChargeReport {
  readonly n: number;
  readonly state: 0 | 1;
  readonly side: number;
  readonly rmsAbs: number;
  readonly rmsRef: number;
  readonly rmsRel: number;
  readonly maxAbs: number;
  readonly depositedUnits: number;
  readonly repeatEqual: boolean;
  /** The device's `f32` min/max, equal bit for bit to the host's over the same positions. */
  readonly boundsExact: boolean;
  readonly pass: boolean;
  readonly failures: readonly string[];
  /** `vendor/architecture` as the browser reported them, for the measurement doc. */
  readonly marks: string;
  readonly fallback: boolean;
  /** The arm's own 1M `maxAbs` guard, 0 elsewhere; reported so a breach names its number. */
  readonly maxAbsGuard: number;
}

/**
 * The charge stage: built once per graph, encodes its dispatches into a given encoder.
 *
 * `encodeDeposit` runs the clear, the bounds fold (when `fold` is set) and the deposit.
 * `encodeTransform` runs the widening, the four transforms and the field read; the probe copies
 * the three read-back columns after it, and the tick reads none of them. The probe submits the
 * two halves separately so the repeat fault has an inside to write into; the tick submits them
 * together. `place` rewrites the uniforms for a new frame and `alpha` (`charge-upload.ts`).
 */
export interface ChargeStage {
  /** The per-node increment the read writes, `2n` components, for the tick's merge. */
  readonly delta: GPUBuffer;
  /** Rewrites the five uniforms for a frame and an `alpha`: the tick's, every tick. */
  place(frame: Frame, alpha: number): void;
  encodeDeposit(encoder: GPUCommandEncoder, fold: boolean): void;
  encodeTransform(encoder: GPUCommandEncoder): void;
}

/** The fault selectors, as the uniform's `fault` field carries them. */
const FAULTS: Readonly<Record<string, number>> = {
  butterfly: 1,
  deposit: 2,
  bounds: 3,
  weight: 4,
};

/** `--break repeat` is not in the uniform: it is a host-side write between two runs. */
const FAULT_REPEAT = "repeat";

/** `params.charge · alpha` at `alpha = 1`, which is what the fixture's deltas are at. */
const CHARGE_ALPHA = -90;

/**
 * Builds the charge stage: uploads the constants, compiles the pipelines, binds the groups.
 *
 * The frame uploaded here is the fixture's own, from the header. The tick overwrites it every
 * tick with the host's frame (`frame.ts`), because the positions move and the frame moves with
 * them; the probe never does.
 */
export function buildChargeStage(
  device: GPUDevice,
  buffers: Buffers,
  fixture: Fixture,
  faultCode: number,
): ChargeStage {
  uploadConstants(device, buffers, fixture);
  const rig = build(device, buffers);
  // The kernel the fixture carries is its own rung's; a frame on another rung refreshes it.
  let rung = rungOf(fixture);
  const place = (frame: Frame, alpha: number): void => {
    const at = { ...frame, charge: CHARGE_ALPHA * alpha, fault: faultCode };
    if (rungOf(frame) !== rung) {
      refreshKernel(device, buffers, rig, { fixture, place: at });
      rung = rungOf(frame);
    }
    writeFrames(device, buffers, fixture, at);
  };
  place(fixture, 1);
  const { side } = fixture;
  return {
    delta: buffers.delta,
    place,
    encodeDeposit(encoder: GPUCommandEncoder, fold: boolean): void {
      // zero_density: the clear is its own dispatch, never the deposit reading what was there.
      let pass = encoder.beginComputePass();
      dispatch(pass, rig.zero, rig.zeroBind, groupsFor(side * side));
      pass.end();
      if (fold) {
        // fold_block then fold_extent: ⌈n/256⌉ blocks, then one invocation over them.
        pass = encoder.beginComputePass();
        dispatch(pass, rig.block, rig.blockBind, groupsFor(fixture.n));
        pass.end();
        pass = encoder.beginComputePass();
        dispatch(pass, rig.extent, rig.extentBind, 1);
        pass.end();
      }
      pass = encoder.beginComputePass();
      dispatch(pass, rig.deposit, rig.depositBind, groupsFor(fixture.n));
      pass.end();
    },
    encodeTransform(encoder: GPUCommandEncoder): void {
      let pass = encoder.beginComputePass();
      dispatch(pass, rig.widen, rig.widenBind, groupsFor(side * side));
      pass.end();
      // The four transforms, one workgroup per line, in mesh.rs's order.
      for (let index = 0; index < rig.fftBind.length; index += 1) {
        const bind = rig.fftBind[index];
        if (bind === undefined) {
          throw new Refusal(`gpu: no bind group for transform pass ${index}`);
        }
        pass = encoder.beginComputePass();
        dispatch(pass, rig.fft, bind, side);
        pass.end();
      }
      pass = encoder.beginComputePass();
      dispatch(pass, rig.read, rig.readBind, groupsFor(fixture.n));
      pass.end();
    },
  };
}

/**
 * The charge pass for one fixture: bounds, deposit, FFT, kernel, field read, compare.
 *
 * `fault` is a *parameter*, not a field of `ChargeRequest`: the public type stays the plan's,
 * so no caller can inject a fault by accident. The harness passes `undefined` for a real run
 * and one of the five names for a control.
 */
export async function runCharge(request: ChargeRequest, fault?: string | null): Promise<ChargeReport> {
  const fixture = loadFixture(request.fixture);
  const host: GpuHost = request.host ?? navigator;
  const repeat = fault === FAULT_REPEAT;
  const code = faultCodeFor(repeat ? undefined : fault);
  const { device, marks, fallback } = await open(host, request.arm ?? "any", fixture);
  try {
    const buffers = create(device, fixture.n, fixture.side, 4);
    try {
      const stage = buildChargeStage(device, buffers, fixture, code);
      const first = await runChargeOnce(device, buffers, stage, fixture);
      // The repeat fault's write lands here: after the second run's deposit and before its
      // transform. Written after the first run instead, the second run's own clear would erase it
      // and the control would prove nothing — which is the bug this fault was written to catch.
      if (repeat) {
        bumpDensity(device, buffers);
      }
      const second = await runChargeOnce(device, buffers, stage, fixture);
      return report(fixture, first, second, marks, fallback);
    } finally {
      destroy(buffers);
    }
  } finally {
    device.destroy();
  }
}

/** The fault's code, or a throw naming the five that exist. */
function faultCodeFor(fault?: string | null): number {
  if (fault === undefined || fault === null) {
    return 0;
  }
  const code = FAULTS[fault];
  if (code === undefined) {
    throw new Refusal(`--break ${fault}: the faults are ${[...Object.keys(FAULTS), FAULT_REPEAT].sort().join(", ")}`);
  }
  return code;
}

/** One full run: the deposit submit, the transform submit, the read-back. */
async function runChargeOnce(
  device: GPUDevice,
  buffers: Buffers,
  stage: ChargeStage,
  fixture: Fixture,
): Promise<Ran> {
  const planeBytes = fixture.side * fixture.side * 4;
  const nodeBytes = fixture.n * 8;
  let encoder = device.createCommandEncoder();
  stage.encodeDeposit(encoder, true);
  device.queue.submit([encoder.finish()]);
  encoder = device.createCommandEncoder();
  stage.encodeTransform(encoder);
  encoder.copyBufferToBuffer(buffers.density, 0, buffers.read.density, 0, planeBytes);
  encoder.copyBufferToBuffer(buffers.delta, 0, buffers.read.delta, 0, nodeBytes);
  encoder.copyBufferToBuffer(buffers.extent, 0, buffers.read.extent, 0, 16);
  device.queue.submit([encoder.finish()]);
  return readback(buffers, planeBytes, nodeBytes);
}

/** What `Kernel::refresh` keys its spectrum on, beside the fixed law (`kernel.rs:48-53`). */
function rungOf(frame: Frame): string {
  return `${frame.step}:${frame.reach}`;
}
