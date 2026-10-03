import type { PaintCounts } from "./canvas2d/input.ts";
import { type Rate, fpsOf } from "./canvas2d/rate.ts";
import type { BulkSlot } from "./webgl2/hook.ts";
import type { ViewStats } from "./view.ts";

/**
 * What a frame's stats are read from: the loop's own fields and nothing else, so a test can hand
 * `statsOf` the counters it reads rather than build a whole loop (canvas2d/loop.ts).
 */
export interface StatsInput {
  readonly counts: PaintCounts;
  readonly scene: { readonly frame: { readonly nodeCount: number; readonly edgeCount: number } };
  readonly sprites: { readonly rasterised: () => number };
  readonly bulk: Pick<BulkSlot, "failure" | "refining">;
  readonly rate: Rate;
  layoutRuns: number;
  frameMs: number;
  frames: number;
}

export function statsOf(state: StatsInput): ViewStats {
  return {
    backend: state.counts.bulk > 0 ? "webgl2" : "canvas2d",
    backendFailure: state.bulk.failure,
    refining: state.bulk.refining,
    nodes: state.scene.frame.nodeCount,
    edges: state.scene.frame.edgeCount,
    drawnNodes: state.counts.nodes,
    drawnEdges: state.counts.edges,
    drawnLabels: state.counts.labels,
    drawnArrows: state.counts.arrows,
    arrowSize: state.counts.arrowSize,
    curvedEdges: state.counts.curves,
    strokeWidth: state.counts.stroke,
    draws: state.counts.draws,
    strokeCalls: state.counts.strokes,
    edgeStyles: state.counts.edgeStyles,
    mixedEdges: state.counts.mixedEdges,
    gradientStrokes: state.counts.gradientStrokes,
    arrowFills: state.counts.arrowFills,
    glowFills: state.counts.glowFills,
    spritesRasterised: state.sprites.rasterised(),
    layoutRuns: state.layoutRuns,
    frameMs: state.frameMs,
    fps: fpsOf(state.rate, performance.now()),
    frames: state.frames,
  };
}