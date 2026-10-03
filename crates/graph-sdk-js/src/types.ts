// `import type` only (C18): this file never pulls a runtime value out of the generated
// contract — a generated file can be regenerated at any time, and the only thing this SDK
// is allowed to depend on from it is shape, checked away entirely by `tsc --noEmit`
// (`docs/contract/wasm-abi.md` "Generated types"). The path is the file
// `graph-cli codegen` actually writes and pins (`crates/graph-contract/src/lib.rs`'s
// `codegen::outputs()`), never a hand-copied duplicate of its interfaces.
import type { EdgeGeometryKind, NodeGeometryKind, SnapshotHeader } from "../../graph-contract/generated/snapshot-header.d.ts";
import type { MotorThreads } from "./threads.ts";

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
  /** Node `z`. 3D only — present iff the run's `dim` is 1, whatever the node kind is.
   *  Appended, so no shipped id is renumbered. */
  NodeZ: 12,
} as const;

/** A `ColumnId` value, e.g. `ColumnId.NodeX`. */
export type ColumnId = (typeof ColumnId)[keyof typeof ColumnId];

/** How many dimensions a run or a pass's result carries: `0` 2D, `1` 3D. The header's
 *  `dim` byte, named: the contract spells it 0 for 2D rather than 2, so that a 0.3
 *  snapshot (whose byte is always 0) still reads as 2D. */
export type Dim = 0 | 1;

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
  /** How many dimensions the run carries: `0` 2D, `1` 3D. A 3D run has a
   *  {@link ColumnId.NodeZ} column; a 2D one reads that column as `null` (absent, not
   *  empty). The motor transports 3D — refusing it is a consumer's decision, and this is
   *  what a consumer reads to make it. */
  readonly dim: Dim;
}

/** What `Motor.post` returns. A POST pass replaces the edge geometry of the handle's
 *  last run and leaves the nodes where they were, so this is the shape `RunResult`
 *  carries — `edgeKind` is the member a caller cannot predict, since which kind a pass
 *  emits is the capability's own declaration. */
export interface PostResult {
  readonly handle: Handle;
  /** The capability that ran, e.g. `"post.route.grid"`. */
  readonly id: string;
  /** The handle's node geometry tag after the pass — unchanged by a pass, restated so a
   *  caller does not have to sequence `gm_geometry_kind` itself. */
  readonly nodeKind: NodeGeometryKind;
  /** The edge geometry tag the pass produced: `0` Line, `1` Polyline, `2` Curve. */
  readonly edgeKind: EdgeGeometryKind;
  readonly nodeCount: number;
  /** The handle's dimension after the pass, restated for the same reason as
   *  {@link RunResult.dim}: a POST pass replaces edge geometry and leaves the nodes, z
   *  included, where they were. */
  readonly dim: Dim;
}

/** The element type of an {@link AnalysisResult}'s `values`, as the ABI's JSON names it:
 *  `f64` for a centrality, `u32` for a labelling, a community id, a depth level or the
 *  degree count. One discriminant rather than two optional arrays, so a consumer narrows
 *  on it and cannot read a `u32` label as if it were a score. */
export type AnalysisValueKind = "f64" | "u32";

/** What `Motor.analysis` returns: the ABI's canonical JSON face, parsed and typed.
 *
 *  The three optional members are present exactly when the analysis hands one back, and
 *  each is the escape hatch that analysis's own `Ponytail` marker names — a caller told
 *  only `values` would read an un-converged eigenvector iteration as a real centrality.
 *  `converged` is the power iteration's residual-verified flag (`false` on a bipartite
 *  or disconnected graph, where the iteration oscillates and never settles);
 *  `modularity` is the quality of the partition `values` names; `max` is the deepest
 *  hierarchy level reached. */
export interface AnalysisResult {
  /** The analysis that produced this, e.g. `"analysis.components.weak"`. */
  readonly id: string;
  /** Nodes in the analysed graph: one `values` entry each. */
  readonly nodeCount: number;
  readonly kind: AnalysisValueKind;
  /** One entry per node, in the motor's dense-index order. */
  readonly values: readonly number[];
  readonly converged?: boolean | undefined;
  readonly modularity?: number | undefined;
  readonly max?: number | undefined;
}

/** `createMotor`'s options. Reserved fields read but not yet acted on are rejected, never
 * silently ignored (C16): a caller who thinks `exec` picked a compute tier must be told it
 * did not, not shipped a motor that quietly ran on the default tier anyway. */
export interface MotorOptions {
  /** Reserved for Phase 11 (`docs/decisions/compute-tiers.md`): the only accepted value
   * this phase is `"auto"`, meaning "whatever this build supports" — the same thing
   * omitting the field means. Any other value is refused by `createMotor` (`InvalidOptionsError`). */
  exec?: "auto";
  /** Load the threads artifact and tick live sessions on its pool (`threads.ts`). Inside a
   * Worker only: the coordinator blocks while its helpers run. */
  threads?: MotorThreads;
}

/** An opaque live force session id, `gm_force_session_create`'s answer
 * (`docs/decisions/force-wasm-abi.md`). Never construct one by hand, and never confuse it
 * with a {@link Handle}: they are two id spaces with two error codes, and a session outlives
 * the graph handle it was created from. */
export type ForceSessionId = number & { readonly __brand: "GraphMotorForceSession" };

/** The force parameters a session can be told, field for field
 * (`graph_core::layout::force::LiveParams`). Every field is range-checked by the motor and
 * **never clamped**, so an out-of-range value is a refusal rather than a quiet clamp — which
 * is why these are plain `number`s with no normalisation applied here either. */
export interface ForceParams {
  /** Many-body repulsion, `-5000..=0`. */
  charge: number;
  /** Barnes-Hut opening angle, `0.3..=1.5`. */
  theta: number;
  distance_min: number;
  distance_max: number;
  /** Base link distance; a link's own is this over `max(0.4, strength)`. */
  link_distance: number;
  link_strength_scale: number;
  collide_radius: number;
  center_strength: number;
  /** Pull toward the origin, `0..=1`. `0` skips the force entirely.
   *
   *  Ponytail: the one knob with no default worth shipping, because every graph wants a
   *  different one. Failing input: a graph whose natural extent is far larger than the
   *  viewport, where nothing else pulls distant structure back. Direction: over-shrinking —
   *  a strong gravity collapses clusters onto the origin, which is visible rather than
   *  silent. Escape hatch: it is `0` by default, and `0` is a skip, not a zero strength. */
  gravity: number;
  /** Per-tick velocity multiplier, `0.01..=0.99` (d3's `velocityDecay` is `1 - this`). */
  velocity_decay: number;
  alpha_decay: number;
  alpha_min: number;
  initial_alpha: number;
}

/** What one `ForceSession#tick` did. `settled` is the motor's own verdict
 * (`alpha < alpha_min` with no target holding it up), not `alpha`'s value re-tested here, so
 * the SDK and the motor cannot disagree about when a layout has stopped moving. */
export interface ForceTick {
  readonly status: ForceStatus;
  readonly alpha: number;
  /** The ticks that ran: the argument, always. A chunked caller adds these up. */
  readonly ticksRun: number;
}

/** The tick a {@link ForceSessionId}'s session runs: Barnes-Hut's quadtree
 * (`layout.force.barnes_hut`), or the particle mesh's FFT grid (`layout.force.particle_mesh`),
 * `O(n)` per tick and the one for graphs past about 50k nodes. The two are different bytes. */
export type ForceEngine = "barnes_hut" | "particle_mesh";

/** The wire's status word, as words: `1` ran and is still cooling, `2` ran and has settled.
 * `0` never reaches here — it is the refusal, and it throws. */
export type ForceStatus = "running" | "settled";
