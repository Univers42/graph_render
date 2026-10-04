/**
 * One run over a built graph, read back as the report the studio draws: the layout, the edge
 * pass, the bytes and their digest; or one analysis. Touches no force session: whether a run
 * renews it is the caller's call (`session.ts`), since a delta batch's snapshot must not.
 */
import { type Built, describe } from "./built.ts";
import type { AnalysisFace, MotorLike, SessionDeps } from "./session.ts";
import { type ShownError, describeError } from "../state/errors.ts";
import type { AnalysisReport, RunReport } from "./protocol.ts";
import type { RunPlan } from "./settle.ts";

export interface Live<Handle> {
  readonly motor: MotorLike<Handle>;
  readonly built: Built<Handle>;
}

type Clock<Handle> = Pick<SessionDeps<Handle>, "now" | "digest">;

interface Pass {
  readonly postId: string | null;
  readonly postError: ShownError | null;
  readonly postMs: number;
}

function runPass<Handle>(motor: MotorLike<Handle>, handle: Handle, postId: string | null, now: () => number): Pass {
  if (postId === null) return { postId, postError: null, postMs: 0 };
  const started = now();
  try {
    motor.post(handle, postId);
    return { postId, postError: null, postMs: now() - started };
  } catch (error) {
    // A refused pass leaves the layout's own edges in place, so the run is still drawn.
    return { postId: null, postError: describeError(error), postMs: now() - started };
  }
}

/** Runs `plan.run` and reports it as `plan.report`, with the edge pass and the digest. */
export async function snapshot<Handle>(
  live: Live<Handle>,
  deps: Clock<Handle>,
  plan: Pick<RunPlan, "run" | "report">,
  postId: string | null,
): Promise<RunReport> {
  const { motor, built } = live;
  const started = deps.now();
  motor.layout(built.handle, plan.run);
  const layoutMs = deps.now() - started;
  const pass = runPass(motor, built.handle, postId, deps.now);
  const bytes = motor.toBytes(built.handle);
  const meta = describe(built, bytes);
  return { layoutId: plan.report, ...pass, bytes, digest: await deps.digest(bytes), layoutMs, meta };
}

function reportOf(face: AnalysisFace, ms: number): AnalysisReport {
  return {
    id: face.id,
    kind: face.kind,
    values: face.kind === "u32" ? Uint32Array.from(face.values) : Float64Array.from(face.values),
    converged: face.converged ?? null,
    modularity: face.modularity ?? null,
    max: face.max ?? null,
    ms,
  };
}

/** One analysis over the graph, in the face the studio reports. */
export function runAnalysis<Handle>(live: Live<Handle>, deps: Clock<Handle>, analysisId: string): AnalysisReport {
  const started = deps.now();
  const face = live.motor.analysis(live.built.handle, analysisId);
  return reportOf(face, deps.now() - started);
}
