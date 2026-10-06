/**
 * `adapter.ts` — the adapter request, the four limits, and the refusal ladder.
 *
 * **The limits are queried, not asserted.** WebGPU's types are *declarations* here
 * (`types.ts`), so the two arms' differences are runtime checks and not compile errors — which
 * is exactly why they have to be checks. Four are needed for this pass and a breach of any is
 * a refusal:
 *
 * | limit | needed | why |
 * |---|---:|---|
 * | `maxComputeInvocationsPerWorkgroup` | 256 | the software arm's is exactly 256; a device under it cannot run this tier at all |
 * | `maxComputeWorkgroupStorageSize` | 8 KB | the FFT's fixed `var<workgroup>` of 1 024 complex samples, inside both arms' 65 536 and 32 768 |
 * | `maxComputeWorkgroupsPerDimension` | `⌈n/256⌉` | 3 907 at 1M, against 65 535 on both arms |
 * | `maxStorageBufferBindingSize` | 8 MB · `⌈n/10⁶⌉` | positions, density, spectrum and scratch; 4 GiB hardware, 1 GiB software |
 *
 * **No `shader-f16` is required and none may be used.** The software arm has no such feature
 * (`gpu-adapter.md:130-131`), so an f16 path would be a hardware-only path by construction.
 * That is enforced on the *source*, by `every_kernel_is_256_wide_and_f32` and the `no-f16` row,
 * and enforced on the *types* by `types.ts` declaring no f16 type at all.
 *
 * The limits check needs the fixture's `n` and `P`, so `open` takes the loaded fixture rather
 * than only a `ChargeRequest`. That is also why it is not part of `probeCharge`'s public shape:
 * a caller who has bytes does not have a device.
 */

import { FFT_WORKGROUP_BYTES } from "./kernels/fft.wgsl.ts";
import type { GPU, GPUAdapter, GPUDevice, GPUSupportedLimits } from "./types.ts";
import { gpuOf } from "./types.ts";

/** A refusal: why this arm will not run, in one line, for the harness's stderr. */
export class Refusal extends Error {
  constructor(message: string) {
    super(message);
    this.name = "Refusal";
  }
}

/** The device and what it said about itself, for the report and the measurement doc. */
export interface Opened {
  readonly device: GPUDevice;
  readonly adapter: GPUAdapter;
  /** `vendor / architecture` as the browser reported them, for the measurements doc. */
  readonly marks: string;
  readonly fallback: boolean;
}

/** Which adapter to insist on, and the `requestAdapter` options that express it. */
function optionsFor(arm: string): { readonly powerPreference: "high-performance"; readonly forceFallbackAdapter: boolean } {
  return { powerPreference: "high-performance", forceFallbackAdapter: arm === "software" };
}

/**
 * Requests the adapter, checks the four limits, and returns the device.
 *
 * `host` is the object carrying `gpu`, passed in rather than read off `navigator` so the test
 * and the page call the same code. Throws a `Refusal` for every way this can decline, and the
 * harness turns that into exit 3 as `webgpu.py:236-250` does.
 */
export async function open(
  host: { readonly gpu?: GPU },
  arm: string,
  needed: { readonly n: number; readonly side: number },
): Promise<Opened> {
  const gpu = gpuOf(host);
  if (!gpu) {
    throw new Refusal("no navigator.gpu, so this browser has no WebGPU at all");
  }
  const adapter = await gpu.requestAdapter(optionsFor(arm));
  if (!adapter) {
    throw new Refusal("requestAdapter returned null, so there is no adapter of any kind");
  }
  checkLimits(adapter.limits, needed);
  const device = await adapter.requestDevice();
  if (!device) {
    throw new Refusal("requestDevice returned nothing, so the adapter gave no device");
  }
  return { device, adapter, marks: marksOf(adapter), fallback: fallbackOf(adapter) };
}

/** The four limit checks, each naming its own limit so a refusal says which one. */
function checkLimits(
  limits: GPUSupportedLimits,
  needed: { readonly n: number; readonly side: number },
): void {
  if (limits.maxComputeInvocationsPerWorkgroup < 256) {
    throw new Refusal(
      `maxComputeInvocationsPerWorkgroup is ${limits.maxComputeInvocationsPerWorkgroup}, and every kernel is 256`,
    );
  }
  if (limits.maxComputeWorkgroupStorageSize < FFT_WORKGROUP_BYTES) {
    throw new Refusal(
      `maxComputeWorkgroupStorageSize is ${limits.maxComputeWorkgroupStorageSize}, and the FFT needs ${FFT_WORKGROUP_BYTES}`,
    );
  }
  const groups = Math.ceil(needed.n / 256);
  if (limits.maxComputeWorkgroupsPerDimension < groups) {
    throw new Refusal(
      `maxComputeWorkgroupsPerDimension is ${limits.maxComputeWorkgroupsPerDimension}, and n=${needed.n} needs ${groups}`,
    );
  }
  const want = 8 * 1024 * 1024 * Math.max(1, Math.ceil(needed.n / 1_000_000));
  if (limits.maxStorageBufferBindingSize < want) {
    throw new Refusal(
      `maxStorageBufferBindingSize is ${limits.maxStorageBufferBindingSize}, and n=${needed.n} needs ${want}`,
    );
  }
}

/** `vendor / architecture`, the two fields this browser build populates. */
function marksOf(adapter: GPUAdapter): string {
  const info = adapter.info ?? {};
  return [info.vendor, info.architecture].filter((part) => !!part).join("/");
}

/** `isFallbackAdapter`, which moved onto `adapter` on some builds and onto `info` on others. */
function fallbackOf(adapter: GPUAdapter): boolean {
  if (adapter.isFallbackAdapter !== undefined) {
    return adapter.isFallbackAdapter;
  }
  return false;
}