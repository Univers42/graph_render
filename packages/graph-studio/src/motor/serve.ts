/** One request in, one answer out. A failure is an answer too: the pump never throws. */
import { NO_ADAPTER_REASON } from "./live.ts";
import { DeltaRefusal } from "./deltas.ts";
import { describeError } from "../state/errors.ts";
import type { ForceHost } from "./liveLoop.ts";
import { type Request, type Result, isForceRequest } from "./protocol.ts";
import type { Session } from "./session.ts";

export interface Answer {
  readonly result: Result;
  /** Buffers the answer hands over instead of copying. */
  readonly transfer: ArrayBufferLike[];
}

/**
 * One batch, answered by the tick that applied it.
 *
 * WHY awaited here and not answered at once: the answer says whether the graph changed, and it
 * cannot say so before the extend and the grow have run. Caveat: the pump answers one request
 * at a time, so a batch waiting for its tick holds the queue for that tick — a tick is one
 * frame, and the frames the loop emits do not go through the pump.
 */
async function answerDeltas(forces: ForceHost | null, batch: Request & { readonly type: "force.deltas" }): Promise<Answer> {
  const result: Result = forces === null
    ? { type: "failed", error: describeError(new DeltaRefusal(NO_ADAPTER_REASON)) }
    : await forces.deltas(batch.batch);
  return { result, transfer: [] };
}

async function answerTo(session: Session, request: Request, forces: ForceHost | null): Promise<Answer> {
  if (request.type === "force.deltas") return answerDeltas(forces, request);
  if (isForceRequest(request)) {
    const result: Result = forces === null
      ? { type: "force-state", running: false, disabled: NO_ADAPTER_REASON, paused: false }
      : forces.handle(request);
    return { result, transfer: [] };
  }
  if (request.type === "open") {
    return { result: { type: "opened", catalog: await session.open(request.wasmUrl, request.threads) }, transfer: [] };
  }
  if (request.type === "load") {
    return { result: { type: "loaded", graph: await session.load(request.source, request.fixturesUrl) }, transfer: [] };
  }
  if (request.type === "layout") {
    const run = await session.layout(request.layoutId, request.postId);
    return { result: { type: "laid-out", run }, transfer: [run.bytes.buffer] };
  }
  const analysis = session.analysis(request.analysisId);
  return { result: { type: "analysed", analysis }, transfer: [analysis.values.buffer] };
}

export async function serve(session: Session, request: Request, forces: ForceHost | null = null): Promise<Answer> {
  try {
    return await answerTo(session, request, forces);
  } catch (error) {
    return { result: { type: "failed", error: describeError(error) }, transfer: [] };
  }
}
