// `import type` only (C18): this file never pulls a runtime value out of the generated
// contract — a generated file can be regenerated at any time, and the only thing this SDK
// is allowed to depend on from it is shape, checked away entirely by `tsc --noEmit`
// (`docs/contract/wasm-abi.md` "Generated types"). The path is the file
// `graph-cli codegen` actually writes and pins (`crates/graph-contract/src/lib.rs`'s
// `codegen::outputs()`), never a hand-copied duplicate of its interfaces.
import type { EdgeGeometryKind, NodeGeometryKind, SnapshotHeader } from "../../graph-contract/generated/snapshot-header.d.ts";

export type { EdgeGeometryKind, NodeGeometryKind, SnapshotHeader };

/** Column ids (`crates/graph-wasm/src/views.rs::id`, `docs/contract/wasm-abi.md`
 * "Columns"): append-only, never renumbered. `NoteCode`/`NoteIndex` are reserved for the
 * two fields Phase 3's notes section brings — `note.code` and `note.index` — and read
 * [`ColumnKind.Absent`](#absent) until that merge, for every graph. */
export const ColumnId = {
  NodeX: 0,
  NodeY: 1,
  NodeR: 2,
  NodeW: 3,
  NodeH: 4,
  EdgeSource: 5,
  EdgeTarget: 6,
  NoteCode: 7,
  NoteIndex: 8,
  EdgeOffsets: 9,
  EdgePts: 10,
  EdgeCurveDegree: 11,
} as const;

/** A `ColumnId` value, e.g. `ColumnId.NodeX`. */
export type ColumnId = (typeof ColumnId)[keyof typeof ColumnId];

/** An opaque handle `gm_build` returned. Never construct one by hand: it is only ever a
 * `number` this SDK itself received back from the motor (C6's monotonic, never-reused id
 * is the motor's own invariant, not something this type can enforce, so the SDK never
 * hands a caller a bare `number` under a different name to reduce the chance of one
 * slipping in from elsewhere). */
export type Handle = number & { readonly __brand: "GraphMotorHandle" };

/** A typed view over one column's live data, or `null` if that column is reserved or does
 * not apply to this snapshot's geometry kind (C3: absent, not a zero-length array — the
 * caller tells the two apart by `nodeKind`/`edgeKind`, not by this being present-but-empty
 * vs. `null`). Aliases the motor's own memory: valid only until the next call on this
 * `Motor` (any handle, not just this one — C7), and never held across it. */
export type Column = Float32Array | Uint32Array | null;

/** What `Motor.layout` returns: everything read back right after a successful run, so a
 * caller does not have to sequence `gm_geometry_kind`/`gm_column_ptr`/... itself. */
export interface RunResult {
  readonly handle: Handle;
  readonly nodeKind: NodeGeometryKind;
  readonly edgeKind: EdgeGeometryKind;
  readonly nodeCount: number;
}

/** `createMotor`'s options. Reserved fields read but not yet acted on are rejected, never
 * silently ignored (C16): a caller who thinks `exec` picked a compute tier must be told it
 * did not, not shipped a motor that quietly ran on the default tier anyway. */
export interface MotorOptions {
  /** Reserved for Phase 11 (`docs/decisions/compute-tiers.md`): the only accepted value
   * this phase is `"auto"`, meaning "whatever this build supports" — the same thing
   * omitting the field means. Any other value is refused by `createMotor` (`InvalidOptionsError`). */
  exec?: "auto";
}
