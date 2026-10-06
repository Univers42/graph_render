/**
 * The GPU tier's face in this SDK: reading a fixture the motor wrote.
 *
 * G1a ships the loader and nothing else — no WGSL, no dispatch, no adapter request. What is
 * here is the half both arms' harnesses need from the start: a `.gmfx` read exactly as
 * `graph-cli emit-gpu-fixtures` wrote it, with no arithmetic of its own.
 *
 * The WGSL and its host arrive in the next slice, under `crates/graph-sdk-js/src/gpu/`. The
 * tier is `layout.force.particle_mesh.gpu`, it is requested explicitly and never substituted
 * for the CPU id, and it is not in graph-core's `LAYOUTS` and not in the hash gate — the
 * degradation is per-device repeatability, not native/wasm equality
 * (`docs/decisions/gpu-force-tier.md:34-38`).
 */

export { loadFixture, scaleFor } from "./gpu/fixture.ts";
export type { Fixture, Pass } from "./gpu/fixture.ts";
