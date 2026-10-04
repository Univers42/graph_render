/**
 * One run over a built graph, read back as the report the studio draws: the layout, the edge
 * pass, the bytes and their digest; or one analysis. Touches no force session: whether a run
 * renews it is the caller's call (`session.ts`), since a delta batch's snapshot must not.
 */
import { type Built, describe } from "./built.ts";
import { runPass } from "./edgePass.ts";
import type { AnalysisFace, MotorLike, SessionDeps } from "./session.ts";
import type { ParamValues } from "../state/settings.ts";
import type { AnalysisReport, RunReport } from "./protocol.ts";
import type { RunPlan } from "./settle.ts";

export interface Live<Handle> {
  readonly motor: MotorLike<Handle>;
  readonly built: Built<Handle>;
}

type Clock<Handle> = Pick<SessionDeps<Handle>, "now" | "digest">;

/** What a snapshot runs with: the edge pass, and the values (empty: the layout's own defaults). */
export interface Shot {
  readonly postId: string | null;
  readonly params: ParamValues;
}

/** Runs `plan.run` at `shot.params` and reports it as `plan.report`, with the edge pass and the digest. */
export async function snapshot<Handle>(
  live: Live<Handle>,
  deps: Clock<Handle>,
  plan: Pick<RunPlan, "run" | "report">,
  shot: Shot,
): Promise<RunReport> {
  const { motor, built } = live;
  const { params } = shot;
  const started = deps.now();
  // No values means no options at all, so the run carries the empty buffer every pre-ABI-2
  // caller sent and the motor's own defaults — the same bytes, not merely the same picture.
  motor.run(built.handle, plan.run, Object.keys(params).length === 0 ? undefined : { params });
  const layoutMs = deps.now() - started;
  const pass = runPass(motor, built.handle, shot.postId, deps.now);
  const bytes = motor.toBytes(built.handle);
  const meta = describe(built, bytes);
  return { layoutId: plan.report, ...pass, params, bytes, digest: await deps.digest(bytes), layoutMs, meta };
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
