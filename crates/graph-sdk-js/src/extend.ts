// `Motor#extend`'s body: a batch of new nodes and edges staged through `gm_alloc` and appended
// to a built graph by `gm_graph_extend` (`docs/contract/delta.md` "The SDK"). A module of its own
// because `motor.ts` is at the house's 300-line limit.
//
// The batch goes through `staging.ts`'s one staging path rather than a second copy of it. That
// path calls `exports[spec.call](ptr, len)`, so it is handed a `gm_build` bound to the graph, as
// `threads.ts` hands the loader a `gm_force_session_tick` of its own: the in-place encoding, the
// free on every path and the epoch bumps are then the build's, and a batch is staged as a
// document is. The inner `invoke` names a trap after the export that trapped, so the two paths
// name their own and neither is reported as the other's.
//
// `extendColumnsGraph` is the same shape over the columnar batch `gm_graph_extend_columns`
// reads: the bytes come from `encodeBatch` instead of `JSON.stringify`, the refusal is
// `ColumnsInvalid` rather than `IngestInvalid` for the same logical fault, and so the class is
// `ColumnsRefusedError` where `extendGraph` throws `BuildRefusedError` — the asymmetry
// `docs/decisions/extend-columns.md` "U1" prices, and `errors.ts`'s class doc states.

import { toU32 } from "./wasm.ts";
import { BuildRefusedError, ColumnsRefusedError, InvalidHandleError } from "./errors.ts";
import { INVALID_HANDLE_CODE, invoke } from "./calls.ts";
import { buildStaged, type StagedBuild } from "./staging.ts";
import { encodeBatch } from "./columns-batch.ts";
import type { ColumnsEdge, ColumnsNode } from "./columns.ts";
import type { StageContext } from "./stages.ts";
import type { Handle } from "./types.ts";

/** New nodes and edges for a built graph, each in the shape {@link Motor.build} reads. An edge
 *  may name a node of the graph or a node of the same batch. */
export interface GraphBatch {
  readonly nodes: readonly ColumnsNode[];
  readonly edges: readonly ColumnsEdge[];
}

const EXTEND_BATCH: StagedBuild = {
  buffer: "batch",
  call: "gm_build",
  payload: "text",
  refusal: "gm_graph_extend refused the batch",
  refuse: (message, code) =>
    code === INVALID_HANDLE_CODE ? new InvalidHandleError(message, code) : new BuildRefusedError(message, code),
};

/** The columnar batch's own refusals. `ColumnsInvalid` covers every format fault *and* every
 *  graph fault here (a repeated id, a dangling endpoint), and `InvalidHandle` is the one code
 *  this ABI shares with `gm_graph_extend`, so it keeps its own class. */
const EXTEND_COLUMNS_BATCH: StagedBuild = {
  buffer: "batch",
  call: "gm_build",
  payload: "bytes",
  refusal: "gm_graph_extend_columns refused the batch",
  refuse: (message, code) =>
    code === INVALID_HANDLE_CODE ? new InvalidHandleError(message, code) : new ColumnsRefusedError(message, code),
};

/** Whether `batch` is shaped like a batch at all. Checked by both paths *before* it is encoded:
 *  `JSON.stringify` would drop a missing member and send a different document, and the columnar
 *  encoder would read `undefined.length`. Each path throws its own class, because that is the
 *  one difference a caller switching between them is owed. */
function isBatch(batch: GraphBatch): boolean {
  return Array.isArray(batch?.nodes) && Array.isArray(batch?.edges);
}

/** Appends `batch` to `handle`'s graph and forgets the handle's last run, which the motor has
 *  cleared (C4). A batch that is not `{ nodes: [], edges: [] }` is refused here, before it is
 *  serialised: `JSON.stringify` would drop a missing member and send a different document. */
export function extendGraph({ exports, views, kinds }: StageContext, handle: Handle, batch: GraphBatch): void {
  if (!isBatch(batch)) {
    throw new BuildRefusedError("the batch must be an object with a nodes array and an edges array");
  }
  const graph = toU32(handle);
  const bound = {
    ...exports,
    gm_build: (ptr: number, len: number) => invoke("gm_graph_extend", () => exports.gm_graph_extend(graph, ptr, len)),
  };
  const document = JSON.stringify({ version: 1, nodes: batch.nodes, edges: batch.edges });
  buildStaged({ exports: bound, views }, document, EXTEND_BATCH);
  kinds.delete(handle);
}

/** Appends `batch` to `handle`'s graph as the `GMX1` document `gm_graph_extend_columns` reads,
 *  encoded here rather than by the host. The same append as {@link extendGraph} over the same
 *  records, and the same invalidation: the last run is cleared by the export, `views` is bumped
 *  by the staging path, and the handle's kind cache is dropped below — a method that appends and
 *  forgets to invalidate it is the quiet failure this repeats deliberately. */
export function extendColumnsGraph({ exports, views, kinds }: StageContext, handle: Handle, batch: GraphBatch): void {
  if (!isBatch(batch)) {
    throw new ColumnsRefusedError("the batch must be an object with a nodes array and an edges array");
  }
  const graph = toU32(handle);
  const bound = {
    ...exports,
    gm_build: (ptr: number, len: number) =>
      invoke("gm_graph_extend_columns", () => exports.gm_graph_extend_columns(graph, ptr, len)),
  };
  buildStaged({ exports: bound, views }, encodeBatch(batch), EXTEND_COLUMNS_BATCH);
  kinds.delete(handle);
}
