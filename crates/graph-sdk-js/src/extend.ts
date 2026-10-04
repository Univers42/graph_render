// `Motor#extend`'s body: a batch of new nodes and edges staged through `gm_alloc` and appended
// to a built graph by `gm_graph_extend` (`docs/contract/delta.md` "The SDK"). A module of its own
// because `motor.ts` is at the house's 300-line limit.
//
// The batch goes through `staging.ts`'s one staging path rather than a second copy of it. That
// path calls `exports[spec.call](ptr, len)`, so it is handed a `gm_build` bound to the graph, as
// `threads.ts` hands the loader a `gm_force_session_tick` of its own: the in-place encoding, the
// free on every path and the epoch bumps are then the build's, and a batch is staged as a
// document is. The inner `invoke` names a trap after `gm_graph_extend`, the export that trapped.

import { toU32 } from "./wasm.ts";
import { BuildRefusedError, InvalidHandleError } from "./errors.ts";
import { INVALID_HANDLE_CODE, invoke } from "./calls.ts";
import { buildStaged, type StagedBuild } from "./staging.ts";
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
  refusal: "gm_graph_extend refused the batch",
  refuse: (message, code) =>
    code === INVALID_HANDLE_CODE ? new InvalidHandleError(message, code) : new BuildRefusedError(message, code),
};

/** Appends `batch` to `handle`'s graph and forgets the handle's last run, which the motor has
 *  cleared (C4). A batch that is not `{ nodes: [], edges: [] }` is refused here, before it is
 *  serialised: `JSON.stringify` would drop a missing member and send a different document. */
export function extendGraph({ exports, views, kinds }: StageContext, handle: Handle, batch: GraphBatch): void {
  if (!Array.isArray(batch?.nodes) || !Array.isArray(batch?.edges)) {
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
