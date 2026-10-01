/** One request in, one answer out. A failure is an answer too: the pump never throws. */
import { NO_ADAPTER_REASON } from "./live.ts";
import { describeError } from "../state/errors.ts";
import type { ForceHost } from "./liveLoop.ts";
import { type Request, type Result, isForceRequest } from "./protocol.ts";
import type { Session } from "./session.ts";

export interface Answer {
  readonly result: Result;
  /** Buffers the answer hands over instead of copying. */
  readonly transfer: ArrayBufferLike[];
}

async function answerTo(session: Session, request: Request, forces: ForceHost | null): Promise<Answer> {
  if (isForceRequest(request)) {
    const result: Result = forces === null
      ? { type: "force-state", running: false, disabled: NO_ADAPTER_REASON, paused: false }
      : forces.handle(request);
    return { result, transfer: [] };
  }
  if (request.type === "open") {
    return { result: { type: "opened", catalog: await session.open(request.wasmUrl) }, transfer: [] };
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
