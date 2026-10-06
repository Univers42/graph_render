/**
 * The GPU tier's face in this SDK: a `.gmfx` reader, and the charge and link passes that run
 * against one.
 *
 * ## What this slice adds, and what it deliberately does not
 *
 * G1a shipped the loader and nothing else. This is the WGSL and its host: the particle mesh's
 * charge pass — bounds, deposit, FFT with the kernel, field read — run on a WebGPU device and
 * compared against the CPU's own per-node velocity increments, which are the fixture's
 * `delta_charge` columns.
 *
 * **Nothing here is exported from `index.ts`.** The tier is `layout.force.particle_mesh.gpu`,
 * it is requested explicitly and never substituted for the CPU id, and it is not in
 * graph-core's `LAYOUTS` and not in the hash gate — the degradation is per-device
 * repeatability, not native/wasm equality (`docs/decisions/gpu-force-tier.md:34-38`). G1d is
 * the slice that offers it as a `ForceEngine` value, and it does that through `types.ts` and
 * `force-create.ts`; widening the SDK's public surface here would make the tier reachable
 * before anything can gate it.
 *
 * ## What the comparison is, precisely
 *
 * The arm reads back its own `f32` per-node increment and compares it against the fixture's
 * `f64` one, narrowed with `Math.fround` at the moment of comparison. That is a **transcription
 * check, not an algorithm check** (condition 6): it catches every GPU-side defect — the
 * deposit's cell and weight indexing, the FFT's butterfly and stage-`half` twiddle indexing, a
 * double-applied `1/P²`, the read's stencil and its fixed cell order, the `charge·alpha` scale,
 * the `f64`→`f32` narrowing, workgroup indexing, the atomics, the transposed read. It cannot
 * see a CPU-side error in the twiddle table, the kernel spectrum, the `1/P²` prescale, the
 * Green's function or the frame ladder: the arms would agree and both be wrong. `perf-mb-fidelity`
 * grades the mesh against the exact all-pairs sum and the hash gate pins the CPU mesh's bytes,
 * and condition 6 requires this doc to say so rather than let "agrees with the CPU mesh" be
 * read as "and the CPU mesh is right".
 *
 * **The link pass** (`gpu/link.ts`, G1c) is the same check for the fixture's `delta_link`: a
 * per-node gather over a CSR built once on the host, no atomics, held to the derived guard
 * `k_measured · 5 · 2⁻²³` and its own measured ceiling (`gpu/bounds-link.ts`). Its public
 * `probeLink` takes no fault, as `probeCharge` takes none.
 *
 * **The bounds are per-device.** `bounds.ts` holds one measured ceiling table keyed by
 * `(arm, n, state)`, and each arm is held to its own row. A driver update re-measures; it does
 * not widen.
 */

export { loadFixture, scaleFor } from "./gpu/fixture.ts";
export type { Fixture, Pass } from "./gpu/fixture.ts";
export { probeCharge } from "./gpu/charge-api.ts";
export type { ChargeReport, ChargeRequest } from "./gpu/charge-api.ts";
export { probeLink } from "./gpu/link-api.ts";
export type { LinkRequest } from "./gpu/link-api.ts";
export { Refusal } from "./gpu/adapter.ts";