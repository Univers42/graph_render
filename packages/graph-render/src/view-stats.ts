import { type LoopState } from "./canvas2d/loop.ts";
import { fpsOf } from "./canvas2d/rate.ts";
import type { ViewStats } from "./view.ts";

export function statsOf(state: LoopState): ViewStats {
  return {
    backend: state.counts.bulk > 0 ? "webgl2" : "canvas2d",
    backendFailure: state.bulk.failure,
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
