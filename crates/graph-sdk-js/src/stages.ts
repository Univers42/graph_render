// The three stages that turn a topology plus a registry id into a result: a layout run, a POST
// pass and an analysis. Split out of `index.ts` so that file holds only the `Motor` surface —
// the class a consumer imports — and this one holds the four blocks that read a registry
// index, call one export, resolve `gm_last_error` into the right class, and record the run's
// geometry kinds.
//
// The three are deliberately **not** folded into one generic helper: `gm_run` takes a params
// pair the other two do not, `gm_post_run` has a refusal (`NoGeometryYet`) the other two do
// not have, and an analysis is the one stage that neither mutates geometry nor records kinds.
// A generic shape over three such steps reads as one abstraction and is three mistakes waiting
// to happen; three blocks of eight lines each is the honest cost of the difference.

import { AnalysisRefusedError, InvalidHandleError, PostRefusedError, codeName } from "./errors.ts";
import { INVALID_HANDLE_CODE, NO_GEOMETRY_CODE, decoder, frame, invoke, lastError, snapshotPtr } from "./calls.ts";
import { parseAnalysisFace } from "./analysis-face.ts";
import { readKinds, type GeometryKinds } from "./geometry-kinds.ts";
import type { ColumnViews } from "./views.ts";
import type { Registries } from "./registries.ts";
import { runAtParams, type LayoutParams, type RunAtParams } from "./params.ts";
import type { AnalysisResult, Handle } from "./types.ts";
import type { RawExports } from "./wasm.ts";

/** What the three stages share: the module, the view epoch, the registries, each layout's
 *  published parameters, and where each handle's last successful run recorded its kinds. Built once per motor, so a stage
 *  is three arguments and not eight. */
export interface StageContext {
  readonly exports: RawExports;
  readonly views: ColumnViews;
  readonly registries: Registries;
  readonly kinds: Map<Handle, GeometryKinds>;
  readonly params: LayoutParams;
}

/** A run's parameter values by published name; `undefined` is the layout's own defaults. */
type RunValues = RunAtParams["values"];

/** What a successful layout run or POST pass leaves behind, restated for the caller so it does
 *  not have to read the kinds back itself (C3: this is what decides whether a column is
 *  present). */
export interface GeometryRun {
  readonly nodeKind: GeometryKinds["nodeKind"];
  readonly edgeKind: GeometryKinds["edgeKind"];
  readonly dim: GeometryKinds["dim"];
}

/** The recorded kinds, for the handle this stage just ran. */
function record(ctx: StageContext, handle: Handle, what: string): GeometryRun {
  const kinds = readKinds(ctx.exports, handle, what);
  ctx.kinds.set(handle, kinds);
  return { nodeKind: kinds.nodeKind, edgeKind: kinds.edgeKind, dim: kinds.dim };
}

/** Runs the registered layout `layoutId` (never a hard-coded index, C1) over `handle`'s
 *  topology, at `values` where given and at the layout's own defaults where not: the
 *  parameter buffer, its schema and its refusal are `params.ts`'s
 *  (`docs/decisions/layout-params.md`). */
export function runLayout(ctx: StageContext, handle: Handle, layoutId: string, values?: RunValues): GeometryRun {
  const { exports, views } = ctx;
  const layoutIndex = ctx.registries.layoutIndex(exports, layoutId);
  const specs = values === undefined ? [] : ctx.params.read(exports, layoutId, layoutIndex);
  runAtParams(exports, views, { handle, layoutIndex, specs, values });
  return record(ctx, handle, layoutId);
}

/** Runs the registered POST capability `postId` over `handle`'s last successful layout run,
 *  **replacing that run's edge geometry**, so the handle's columns read the new edges and the
 *  pass is invisible to the transport. A refused pass leaves the geometry exactly as it was,
 *  so a failed call never serves a half-applied drawing. */
export function runPost(ctx: StageContext, handle: Handle, postId: string): GeometryRun {
  const { exports, views } = ctx;
  const index = ctx.registries.postIndex(exports, postId);
  const ok = invoke("gm_post_run", () => exports.gm_post_run(handle, index));
  views.bump();
  if (ok !== 1) {
    const code = lastError(exports);
    if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, code);
    if (code === NO_GEOMETRY_CODE) {
      throw new PostRefusedError(`handle ${handle} has no successful layout run to draw over`, code);
    }
    throw new PostRefusedError(`gm_post_run refused (${codeName(code)})`, code);
  }
  return record(ctx, handle, postId);
}

/** Runs the registered analysis `analysisId` over `handle`'s topology and returns its typed
 *  result. **No layout run is required** — every analysis is a function of the topology, so
 *  this works straight after a build; only an unknown handle or an unregistered id throws.
 *  Records no geometry: an analysis never touches any. */
export function runAnalysis(ctx: StageContext, handle: Handle, analysisId: string): AnalysisResult {
  const { exports } = ctx;
  const index = ctx.registries.analysisIndex(exports, analysisId);
  const ptr = invoke("gm_analysis_run", () => exports.gm_analysis_run(handle, index));
  if (ptr === 0) {
    const code = lastError(exports);
    if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, code);
    throw new AnalysisRefusedError(`gm_analysis_run refused (${codeName(code)})`, code);
  }
  return parseAnalysisFace(decoder.decode(frame(exports, ptr)), analysisId);
}

/** Nodes in `handle`'s topology — available straight after a build, before any run. `0` is
 *  ambiguous on the wire (a genuinely empty graph, or an invalid handle, C4), so this
 *  resolves it through `gm_last_error` and only the real refusal throws. */
export function nodeCount(exports: RawExports, handle: Handle): number {
  const count = invoke("gm_node_count", () => exports.gm_node_count(handle));
  if (count !== 0) return count;
  if (lastError(exports) === INVALID_HANDLE_CODE) {
    throw new InvalidHandleError(`handle ${handle} is not live`, INVALID_HANDLE_CODE);
  }
  return count;
}

/** The canonical JSON face of `handle`'s last run. Refuses with `TamperedGeometryError` if a
 *  column view wrote a non-finite value into the motor's own buffers since that run (D9
 *  re-validation, C8) — this is what makes writing NaN through a zero-copy view an error
 *  here, not a value that silently reaches JSON. */
export function snapshotText(exports: RawExports, handle: Handle): string {
  return decoder.decode(frame(exports, snapshotPtr(exports, "gm_snapshot_json", handle)));
}

/** The binary face of `handle`'s last run. Same D9 re-validation as {@link snapshotText}. */
export function snapshotBytes(exports: RawExports, handle: Handle): Uint8Array {
  return frame(exports, snapshotPtr(exports, "gm_snapshot_bytes", handle));
}
