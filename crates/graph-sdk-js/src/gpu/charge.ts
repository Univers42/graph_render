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
 * zero_density · fold_block · fold_extent · deposit · widen_density
 * fft_line × 4  (rows fwd, cols fwd, rows×gain inv, cols inv)
 * read_field
 * ```
 *
 * `live` per transform pass is the one `fft/pass.rs` gives and not another: `cells` for the
 * forward rows and the inverse columns, `side` for the other two (`fft/pass.rs:112-125`).
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
import { loadFixture, scaleFor } from "./fixture.ts";
import type { Fixture } from "./fixture.ts";
import type { Arm } from "./bounds.ts";
import { bumpDensity, readback, report } from "./readback.ts";
import type { Ran } from "./readback.ts";
import { build, dispatch, groupsFor } from "./pipelines.ts";
import type { Rig } from "./pipelines.ts";
import type { GPUBuffer, GPUDevice, GpuHost } from "./types.ts";

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
 * The charge pass for one fixture: bounds, deposit, FFT, kernel, field read, compare.
 *
 * `fault` is a *parameter*, not a field of `ChargeRequest`: the public type stays the plan's,
 * so no caller can inject a fault by accident. The harness passes `undefined` for a real run
 * and one of the five names for a control.
 */
export async function runCharge(request: ChargeRequest, fault?: string): Promise<ChargeReport> {
  const fixture = loadFixture(request.fixture);
  const host: GpuHost = request.host ?? navigator;
  const repeat = fault === FAULT_REPEAT;
  const code = faultCodeFor(repeat ? undefined : fault);
  const { device, marks, fallback } = await open(host, request.arm ?? "any", fixture);
  try {
    const buffers = create(device, fixture.n, fixture.side, 4);
    try {
      upload(device, buffers, fixture, code);
      const rig = build(device, buffers);
      const first = await once(device, buffers, rig, fixture);
      if (repeat) {
        bumpDensity(device, buffers);
      }
      const second = await once(device, buffers, rig, fixture);
      return report(fixture, first, second, marks, fallback);
    } finally {
      destroy(buffers);
    }
  } finally {
    device.destroy();
  }
}

/** The fault's code, or a throw naming the five that exist. */
function faultCodeFor(fault?: string): number {
  if (fault === undefined) {
    return 0;
  }
  const code = FAULTS[fault];
  if (code === undefined) {
    throw new Refusal(`--break ${fault}: the faults are ${[...Object.keys(FAULTS), FAULT_REPEAT].sort().join(", ")}`);
  }
  return code;
}

/** The `f32` uploads. Every narrowing in the whole pass happens here, once. */
function upload(device: GPUDevice, buffers: Buffers, fixture: Fixture, faultCode: number): void {
  const { side } = fixture;
  device.queue.writeBuffer(buffers.nodes, 0, pairs(fixture.posX, fixture.posY));
  device.queue.writeBuffer(buffers.twiddle, 0, pairs(fixture.twiddleRe, fixture.twiddleIm));
  const gain = new Float32Array(side * side * 2);
  for (let k = 0; k < side * side; k += 1) {
    gain[k * 2] = Math.fround(fixture.spectrumRe[k] ?? 0);
    gain[k * 2 + 1] = Math.fround(fixture.spectrumIm[k] ?? 0);
  }
  device.queue.writeBuffer(buffers.gain, 0, gain);
  // The stage uniforms. `live`, `mode` and `inverse` are the only per-pass fields and the four
  // transforms each get their own buffer, because the CPU's two transforms take a different
  // `live` count for each of their two passes (fft/pass.rs:112-125).
  writeFrame(device, buffers.frame, fixture, faultCode, fixture.cells, 0, 0);
  const plan: readonly (readonly [number, number])[] = [
    [0, fixture.cells],
    [2, fixture.side],
    [1, fixture.side],
    [2, fixture.cells],
  ];
  for (let index = 0; index < plan.length; index += 1) {
    const entry = plan[index];
    const buffer = buffers.framePass[index];
    if (entry === undefined || buffer === undefined) {
      throw new Refusal(`gpu: no uniform for transform pass ${index}`);
    }
    const [mode, live] = entry;
    // inverse is 1 for the two passes of Fft::inverse, which are plan[2] and plan[3].
    const inverse = index < 2 ? 0 : 1;
    writeFrame(device, buffer, fixture, faultCode, live, mode, inverse);
  }
}

/** Two `Float64Array`s interleaved and narrowed to `f32`, `x` then `y`. */
function pairs(x: Float64Array, y: Float64Array): Float32Array {
  const out = new Float32Array(x.length * 2);
  for (let k = 0; k < x.length; k += 1) {
    out[k * 2] = Math.fround(x[k] ?? 0);
    out[k * 2 + 1] = Math.fround(y[k] ?? 0);
  }
  return out;
}

/**
 * One 64-byte uniform, sixteen 4-byte members, in the prelude's field order.
 *
 * `inv_scale` is `1/scale` and is exact: `scaleFor` returns a power of two, so the widening in
 * `widen_density` is a single exact multiply rather than a division per sample.
 */
function writeFrame(
  device: GPUDevice,
  buffer: GPUBuffer,
  fixture: Fixture,
  faultCode: number,
  live: number,
  mode: number,
  inverse: number,
): void {
  const words = new ArrayBuffer(64);
  const ints = new Uint32Array(words);
  const floats = new Float32Array(words);
  const scale = scaleFor(fixture.n) ?? 1;
  ints[0] = fixture.n;
  ints[1] = fixture.side;
  ints[2] = Math.log2(fixture.side);
  ints[3] = fixture.cells;
  ints[4] = scale;
  ints[5] = Math.ceil(fixture.n / 256);
  ints[6] = live;
  ints[7] = mode;
  ints[8] = inverse;
  ints[9] = faultCode;
  ints[10] = 0;
  floats[11] = fixture.h;
  floats[12] = fixture.originX;
  floats[13] = fixture.originY;
  floats[14] = CHARGE_ALPHA;
  floats[15] = 1 / scale;
  device.queue.writeBuffer(buffer, 0, words);
}

/** One full pass: clear, fold, deposit, widen, four transforms, read. Then the read-back. */
async function once(device: GPUDevice, buffers: Buffers, rig: Rig, fixture: Fixture): Promise<Ran> {
  const { n, side } = fixture;
  const plane = groupsFor(side * side);
  const nodes = groupsFor(n);
  const planeBytes = side * side * 4;
  const nodeBytes = n * 8;
  const encoder = device.createCommandEncoder();
  // zero_density: the clear is its own dispatch, never the deposit reading what was there.
  let pass = encoder.beginComputePass();
  dispatch(pass, rig.zero, rig.zeroBind, plane);
  pass.end();
  // fold_block then fold_extent: ⌈n/256⌉ blocks, then one invocation over them.
  pass = encoder.beginComputePass();
  dispatch(pass, rig.block, rig.blockBind, nodes);
  pass.end();
  pass = encoder.beginComputePass();
  dispatch(pass, rig.extent, rig.extentBind, 1);
  pass.end();
  pass = encoder.beginComputePass();
  dispatch(pass, rig.deposit, rig.depositBind, nodes);
  pass.end();
  pass = encoder.beginComputePass();
  dispatch(pass, rig.widen, rig.widenBind, plane);
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
  dispatch(pass, rig.read, rig.readBind, nodes);
  pass.end();
  encoder.copyBufferToBuffer(buffers.density, 0, buffers.read.density, 0, planeBytes);
  encoder.copyBufferToBuffer(buffers.delta, 0, buffers.read.delta, 0, nodeBytes);
  encoder.copyBufferToBuffer(buffers.extent, 0, buffers.read.extent, 0, 16);
  device.queue.submit([encoder.finish()]);
  return readback(buffers, planeBytes, nodeBytes);
}

