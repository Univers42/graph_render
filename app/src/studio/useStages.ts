/**
 * The two stage overlays: a POST pass on the handle's edge geometry, and an
 * analysis over the handle's topology.
 *
 * Both act on the ONE live handle the runner built, and both are explicit
 * actions with an explicit undo. Two decisions worth naming:
 *
 * - **Clearing a pass re-runs the layout.** The motor has no "un-post": a pass
 *   OVERWROTE the handle's edge columns, and the layout's own geometry is not
 *   reachable again except by running it (`docs/contract/wasm-abi.md` "POST"). So
 *   Clear is a second `layout` call on the same handle, and the studio says so in
 *   the panel rather than pretending the old drawing is still in there.
 * - **An analysis is a function of the topology, not of the pass.** It is
 *   therefore not re-run when a pass is applied or cleared, and it survives a
 *   layout change; it is cleared only when the DOCUMENT changes, because a face
 *   over a different graph would colour nodes by another graph's values.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import { coversGraph, fillsFor } from "../core/analysis.ts";
import type { AnalysisResult } from "../../../crates/graph-sdk-js/src/index.ts";
import type { MotorSession, RunReport } from "../motor/session.ts";

export interface StagesState {
  /** The pass chosen in the picker, whether or not it has been applied. */
  readonly postId: string | null;
  /** The analysis chosen in the picker, whether or not it has been run. */
  readonly analysisId: string | null;
  /** The applied face, or `null`. */
  readonly analysis: AnalysisResult | null;
  /** One fill per node from `analysis`, or `null` — what the canvas paints with. */
  readonly fills: readonly string[] | null;
  readonly setPostId: (id: string) => void;
  readonly applyPost: (id: string) => void;
  readonly clearPost: () => void;
  readonly setAnalysisId: (id: string) => void;
  readonly runAnalysis: (id: string) => void;
  readonly clearAnalysis: () => void;
}

export interface StagesArgs {
  readonly session: MotorSession | null;
  /** The run on screen: a pass runs on the SAME handle as this one. */
  readonly run: RunReport | null;
  /** The runner's setter, so a pass replaces the report the canvas draws. */
  readonly onRun: (report: RunReport) => void;
  /** Bumped per document: a face over another graph must not outlive it. */
  readonly generation: number;
  readonly fail: (error: unknown) => void;
}

export function useStages(args: StagesArgs): StagesState {
  const { session, run, onRun, generation, fail } = args;
  const [postId, setPostId] = useState<string | null>(null);
  const [analysisId, setAnalysisId] = useState<string | null>(null);
  const [analysis, setAnalysis] = useState<AnalysisResult | null>(null);

  // A new document invalidates both overlays. Declared before the actions so a
  // fresh document never shows a face over the previous graph for one commit.
  useEffect(() => {
    setAnalysis(null);
    setPostId(null);
    setAnalysisId(null);
  }, [generation]);

  const applyPost = useCallback(
    (id: string) => {
      if (session === null || run === null) return;
      try {
        onRun(session.post(run, id));
      } catch (error) {
        fail(error);
      }
    },
    [fail, onRun, run, session],
  );

  const clearPost = useCallback(() => {
    if (session === null || run === null) return;
    try {
      // The layout again on the same handle: the only way back to its geometry.
      onRun(session.run(run.layoutId, run.buildMs));
    } catch (error) {
      fail(error);
    }
  }, [fail, onRun, run, session]);

  const runAnalysis = useCallback(
    (id: string) => {
      if (session === null || id === "") return;
      try {
        setAnalysisId(id);
        setAnalysis(session.analysis(id));
      } catch (error) {
        fail(error);
      }
    },
    [fail, session],
  );

  const clearAnalysis = useCallback(() => setAnalysis(null), []);

  // Memoised on the FACE, not recomputed per render: `fills` reaches the canvas
  // through `useRunData`'s dependency list, and a fresh array every render would
  // re-run `setData` — which re-fits the camera — on every unrelated state change
  // in the studio (a hover, an error, a panel opening).
  //
  // The SAME `coversGraph` gate the panel applies decides here, so the canvas and
  // the panel can never disagree: a face that does not describe the run on screen
  // paints nothing, and the panel says why.
  const nodeCount = run?.nodeCount ?? null;
  const fills = useMemo(
    () => (analysis === null || nodeCount === null || !coversGraph(analysis, nodeCount) ? null : fillsFor(analysis)),
    [analysis, nodeCount],
  );

  return {
    postId, analysisId, analysis, fills,
    setPostId, applyPost, clearPost,
    setAnalysisId, runAnalysis, clearAnalysis,
  };
}
