// The published entry point (`docs/contract/wasm-abi.md` "SDK surface"; `harness/sdk-smoke.mjs`
// imports only this file, never `wasm.ts`/`views.ts`/`motor.ts` directly). This file is the
// *promise*: one name a consumer imports, and the whole surface behind it. The class it
// publishes is `motor.ts`, the stages that class delegates to are `stages.ts`, and every
// re-export below is a name the docs, the README or a harness file already imports from here
// — so the public surface is exactly what it was, stated in one list instead of scattered
// through a class body that also had to hold the loader's policy.
//
// `docs/contract/wasm-abi.md` recorded this file as an un-splittable 554-line deviation; it is
// now this barrel plus `motor.ts` (297), which is the house answer the deviation claimed was
// unavailable: split into child modules, never compress. Nothing here is compressed — the
// prose moved with the code it explains.

export type { WasmSource } from "./wasm.ts";
export { resetForTests } from "./wasm.ts";
export { serveHelper, type HelperStart, type MotorThreads } from "./threads.ts";
export * from "./errors.ts";
export * from "./types.ts";
export { ForceSession, PARAMS_BYTES, encodeParams, decodeParams } from "./force.ts";
export { Motor, createMotor } from "./motor.ts";
export { encodeColumns, ColumnsEncoderError } from "./columns.ts";
export type { ColumnsDocument, ColumnsEdge, ColumnsNode } from "./columns.ts";
export { assembleColumns } from "./columns-assemble.ts";
export type { ColumnRows } from "./columns-assemble.ts";

export { parseAnalysisFace } from "./analysis-face.ts";
export * from "./adapters.ts";
export * from "./params.ts";
