/**
 * The WebGPU surface this GPU tier reads, declared locally.
 *
 * Nothing is imported: `@webgpu/types` is not a dependency of the root `package.json` and
 * the decision record forbids moving it or the lockfile
 * (`docs/decisions/gpu-force-tier.md:43-45`). The alternative — declaring them globally — is
 * worse: a global `GPUBuffer` would be ambient in every file of the SDK, and a declaration the
 * tier never reads is a declaration that can be wrong about a limit
 * (plan `:1162`).
 *
 * So this is the **subset the charge pass actually touches**, and the shape follows
 * `deploy/perf/webgpu.py`, the probe that measured both arms on this host: `adapter.info`,
 * `isFallbackAdapter`, the five limits the probe printed, and the buffer/pipeline/device
 * surface the compute canary used (`webgpu.py:54-58`, `:98-130`).
 *
 * Two declarations are worth naming.
 *
 * - **`GPUAdapterInfo` is partial.** Chromium on this build leaves `device` and `description`
 *   empty and exposes only `vendor` and `architecture`
 *   (`docs/measurements/gpu-adapter.md:92-95`), so every field is optional and the reader is
 *   `string | undefined` throughout. A G1 log cannot name a card by its marketing string from
 *   this alone, and the type says so rather than the report.
 * - **There is no half-precision type and no half-precision feature string.** The software arm
 *   exposes no such feature (`gpu-adapter.md:130-131`), so a 16-bit-float path would be a
 *   hardware-only path by construction. Omitting the type is what makes that a compile error
 *   instead of a discovery on someone else's device. The gate row that rules this greps this
 *   whole directory for the three-letter token case-insensitively, prose included, which is why
 *   this comment spells it out instead.
 */

/** What `adapter.info` may hold: every field, absent, on this browser build. */
export interface GPUAdapterInfo {
  readonly vendor?: string;
  readonly architecture?: string;
  readonly device?: string;
  readonly description?: string;
}

/** The five limits the charge pass queries before it dispatches anything. */
export interface GPUSupportedLimits {
  readonly maxStorageBufferBindingSize: number;
  readonly maxBufferSize: number;
  readonly maxComputeWorkgroupStorageSize: number;
  readonly maxComputeInvocationsPerWorkgroup: number;
  readonly maxComputeWorkgroupsPerDimension: number;
}

/** An adapter, and the device it will hand out. */
export interface GPUAdapter {
  readonly info?: GPUAdapterInfo;
  readonly limits: GPUSupportedLimits;
  readonly features: ReadonlySet<string>;
  readonly isFallbackAdapter?: boolean;
  requestDevice(): Promise<GPUDevice>;
}

/** The flags `requestAdapter` takes. */
export interface GPURequestAdapterOptions {
  readonly powerPreference?: "low-power" | "high-performance";
  readonly forceFallbackAdapter?: boolean;
}

/** A device: the buffers, the pipelines and the one queue everything is submitted on. */
export interface GPUDevice {
  readonly limits: GPUSupportedLimits;
  readonly features: ReadonlySet<string>;
  readonly queue: GPUQueue;
  createBuffer(descriptor: GPUBufferDescriptor): GPUBuffer;
  createShaderModule(descriptor: GPUShaderModuleDescriptor): GPUShaderModule;
  createBindGroup(descriptor: GPUBindGroupDescriptor): GPUBindGroup;
  createComputePipeline(descriptor: GPUComputePipelineDescriptor): GPUComputePipeline;
  createCommandEncoder(): GPUCommandEncoder;
  destroy(): void;
}

/** The queue: uploads and submissions, in that order. */
export interface GPUQueue {
  writeBuffer(buffer: GPUBuffer, offset: number, data: BufferSource): void;
  submit(buffers: readonly GPUCommandBuffer[]): void;
}

/** The buffer usage bits this tier sets. Storage, copy source and copy destination. */
export const GPUBufferUsage: {
  readonly MAP_READ: number;
  readonly COPY_SRC: number;
  readonly COPY_DST: number;
  readonly STORAGE: number;
  readonly UNIFORM: number;
} = {
  MAP_READ: 0x0001,
  COPY_SRC: 0x0004,
  COPY_DST: 0x0008,
  STORAGE: 0x0080,
  UNIFORM: 0x0040,
};

/** `mapAsync`'s only mode here: read. */
export const GPUMapMode: { readonly READ: number } = { READ: 0x0001 };

/** How a buffer is to be used. */
export interface GPUBufferDescriptor {
  readonly label: string;
  readonly size: number;
  readonly usage: number;
}

/** A buffer. `getMappedRange` hands out the bytes `mapAsync` resolved. */
export interface GPUBuffer {
  readonly size: number;
  readonly label: string;
  mapAsync(mode: number, offset?: number, size?: number): Promise<void>;
  getMappedRange(offset?: number, size?: number): ArrayBuffer;
  unmap(): void;
  destroy(): void;
}

/** A compiled shader module. Compilation is a promise, and its rejection is a refusal. */
export interface GPUShaderModule {
  readonly label?: string;
  getCompilationInfo?(): Promise<{ readonly messages: readonly { readonly type: string; readonly message: string }[] }>;
}

/** One shader module's request. */
export interface GPUShaderModuleDescriptor {
  readonly label: string;
  readonly code: string;
}

/** What a compute pipeline is built from. */
export interface GPUComputePipelineDescriptor {
  readonly label: string;
  readonly layout: "auto";
  readonly compute: { readonly module: GPUShaderModule; readonly entryPoint: string };
}

/** A compute pipeline, and the bind group layout its `auto` layout derived. */
export interface GPUComputePipeline {
  readonly label?: string;
  getBindGroupLayout(index: number): GPUBindGroupLayout;
}

/** A bind group layout. Read only for what a `GPUBindGroup` is checked against. */
export interface GPUBindGroupLayout {
  readonly label?: string;
}

/** One resource in a bind group. */
export interface GPUBindGroupEntry {
  readonly binding: number;
  readonly resource: { readonly buffer: GPUBuffer };
}

/** A bind group's request. */
export interface GPUBindGroupDescriptor {
  readonly label: string;
  readonly layout: GPUBindGroupLayout;
  readonly entries: readonly GPUBindGroupEntry[];
}

/** A bind group: the resources one pipeline's shader may read. */
export interface GPUBindGroup {
  readonly label?: string;
}

/** A compute pass encoder, live between `beginComputePass` and `end`. */
export interface GPUComputePassEncoder {
  setPipeline(pipeline: GPUComputePipeline): void;
  setBindGroup(index: number, bindGroup: GPUBindGroup): void;
  dispatchWorkgroups(x: number, y?: number, z?: number): void;
  end(): void;
}

/** A command encoder: passes, then copies, then a finished buffer. */
export interface GPUCommandEncoder {
  beginComputePass(): GPUComputePassEncoder;
  copyBufferToBuffer(source: GPUBuffer, sourceOffset: number, destination: GPUBuffer, destinationOffset: number, size: number): void;
  finish(): GPUCommandBuffer;
}

/** One submitted command buffer. */
export interface GPUCommandBuffer {
  readonly label?: string;
}

/** `navigator.gpu`, when the browser has it. */
export interface GPU {
  requestAdapter(options?: GPURequestAdapterOptions): Promise<GPUAdapter | null>;
}

/**
 * The object the arm reads `gpu` off: `navigator` in a page, a literal in a node test.
 *
 * It is a structural type rather than `Navigator`, so a node test needs no shim and no
 * assertion — the house bans `as` in this code, and a shim would be an assertion with extra
 * steps.
 */
export interface GpuHost {
  readonly gpu?: GPU;
}

declare global {
  interface Navigator {
    /**
     * The WebGPU entry point.
     *
     * Absent on a browser without WebGPU, which is why every read is a `?.` and every absence
     * is a `Refusal` rather than a crash. The DOM lib does not declare this member, and this
     * augmentation is the only global type this tier adds — it is exactly what
     * `@webgpu/types` would have done, minus the dependency the decision record forbids
     * (`gpu-force-tier.md:43-45`).
     */
    readonly gpu?: GPU;
  }
}

/** The host's `gpu`, or `null` when the browser has none. */
export function gpuOf(host: GpuHost): GPU | null {
  return host.gpu ?? null;
}