// The two stages downstream of LAYOUT — POST (`gm_post_*`) and ANALYSIS
// (`gm_analysis_*`) — and the refusals each one can report. Split out of `index.ts` by the
// house's 300-line limit, and the same split `crates/graph-wasm/src/exports/mod.rs` makes
// for the same two stages: neither is graph lifecycle, and neither is a read-back.
//
// Neither stage range-checks anything itself: the motor refuses, and the refusal arrives
// here as a `Code` to be named. That is the whole job of this module — a POST pass on a
// handle with no layout run is `NoGeometryYet` and a pass on a dead handle is
// `InvalidHandle`, and a caller must be able to tell those apart.

import { toU32, type RawExports } from "./wasm.ts";
import { AnalysisRefusedError, InvalidHandleError, PostRefusedError, RunRefusedError, codeName } from "./errors.ts";
import { INVALID_HANDLE_CODE, NO_GEOMETRY_CODE, decoder, frame, invoke, lastError } from "./calls.ts";
import { parseAnalysisFace } from "./analysis-face.ts";
import type { ColumnViews } from "./views.ts";
import type { GeometryKinds } from "./geometry-kinds.ts";
import type { AnalysisResult, Handle } from "./types.ts";

/** What one POST call needs beyond the module. */
export interface PostRun {
  /** The graph to draw over: it must have a successful layout run. */
  readonly handle: Handle;
  /** The capability id, for the refusal message and the caller's own bookkeeping. */
  readonly postId: string;
  /** `gm_post_run`'s `post_index`: the registry index `postId` resolved to. */
  readonly postIndex: number;
}

/** Runs the POST capability at `run.postIndex`, and returns the geometry kinds the pass
 *  left behind — the edge geometry it replaced, the node positions it did not.
 *
 *  A refused pass leaves the handle's geometry exactly as it was, so a failed call never
 *  serves a half-applied drawing. */
export function postRun(exports: RawExports, views: ColumnViews, run: PostRun): GeometryKinds {
  const ok = invoke("gm_post_run", () => exports.gm_post_run(toU32(run.handle), toU32(run.postIndex)));
  views.bump();
  if (ok !== 1) throw postRefusal(run, lastError(exports));
  return views.record(run.handle, run.postId);
}

/** The POST refusal, named: a dead handle and a handle with nothing to draw over are two
 *  different mistakes, and one code for both would send a caller looking in the wrong
 *  place. */
function postRefusal(run: PostRun, code: number): Error {
  if (code === INVALID_HANDLE_CODE) return new InvalidHandleError(`handle ${run.handle} is not live`, code);
  if (code === NO_GEOMETRY_CODE) {
    return new PostRefusedError(`handle ${run.handle} has no successful layout run to draw over`, code);
  }
  return new PostRefusedError(`gm_post_run refused (${codeName(code)})`, code);
}

/** What one analysis call needs beyond the module. */
export interface AnalysisRun {
  /** The graph to report on. */
  readonly handle: Handle;
  /** The capability id, which is also the key the parsed report is read under. */
  readonly analysisId: string;
  /** `gm_analysis_run`'s `index`: the registry index `analysisId` resolved to. */
  readonly analysisIndex: number;
}

/** Runs the analysis at `run.analysisIndex` over `handle`'s topology and returns its typed
 *  report. **No layout run is required** — every analysis is a function of the topology —
 *  so this works straight after `Motor#build`; only an unknown handle or an unregistered id
 *  throws. */
export function analysisRun(exports: RawExports, run: AnalysisRun): AnalysisResult {
  const ptr = invoke("gm_analysis_run", () => exports.gm_analysis_run(toU32(run.handle), toU32(run.analysisIndex)));
  if (ptr === 0) {
    const code = lastError(exports);
    if (code === INVALID_HANDLE_CODE) {
      throw new InvalidHandleError(`handle ${run.handle} is not live`, code);
    }
    throw new AnalysisRefusedError(`gm_analysis_run refused (${codeName(code)})`, code);
  }
  return parseAnalysisFace(decoder.decode(frame(exports, ptr)), run.analysisId);
}

/** Why `gm_run` refused, named. Kept beside the two stage refusals it shares a shape
 *  with, so every ABI refusal this SDK maps is in one file rather than three.
 *
 *  A parameter refusal (`ParamOutOfRange` and the rest) arrives here as its code and
 *  leaves as a `RunRefusedError` carrying it: the motor, not this SDK, decided the value
 *  was out of range, and a second rule here would be a second thing to keep in step with
 *  the schema (`docs/decisions/layout-params.md`). */
export function runRefusal(handle: Handle, code: number): Error {
  if (code === INVALID_HANDLE_CODE) return new InvalidHandleError(`handle ${handle} is not live`, code);
  return new RunRefusedError(`gm_run refused (${codeName(code)})`, code);
}