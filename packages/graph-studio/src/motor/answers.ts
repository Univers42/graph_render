/** The answer a request wanted, out of whatever the worker sent back, or a refusal naming it. */
import type { ParamValues } from "../state/settings.ts";
import type { AnalysisReport, GraphSummary, LayoutParamSpec, Result, RunReport } from "./protocol.ts";

export function mismatch(wanted: string, result: Result): Error {
  return new Error(`the motor answered ${result.type} to a request for ${wanted}`);
}

/** The one answer of a wanted kind, or a refusal naming what came back instead. */
export function loaded(result: Result): GraphSummary {
  if (result.type !== "loaded") throw mismatch("load", result);
  return result.graph;
}

/** The schema of the layout asked about; a motor that names another layout is not the answer. */
export function published(result: Result, layoutId: string): readonly LayoutParamSpec[] {
  if (result.type !== "params") throw mismatch("params", result);
  if (result.layoutId !== layoutId) throw new Error(`the motor answered the schema of ${result.layoutId} to a request for ${layoutId}`);
  return result.specs;
}

export function laidOut(result: Result): RunReport {
  if (result.type !== "laid-out") throw mismatch("layout", result);
  return result.run;
}

export function analysed(result: Result): AnalysisReport {
  if (result.type !== "analysed") throw mismatch("analysis", result);
  return result.analysis;
}

/**
 * A run's values, as the request carries them: nothing at all where there are none, so a run
 * with no values of its own sends no `params` member and the motor takes its own defaults.
 */
export function asked(params: ParamValues | undefined): { readonly params?: ParamValues } {
  return params === undefined || Object.keys(params).length === 0 ? {} : { params };
}
