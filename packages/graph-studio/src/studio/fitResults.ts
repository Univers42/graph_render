/** Frames the camera on the nodes the search highlighted, with the look's slack around them. */
import { type Bounds, type FitArea, fitCamera } from "../../../graph-render/src/camera.ts";
import { boundsOfVisible } from "../../../graph-render/src/local.ts";
import { FIT_MARGIN } from "../../../graph-render/src/look/presets.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Outcome } from "../actions/registry.ts";
import { highlightOf } from "../look/visibleOf.ts";
import type { StudioState } from "../state/model.ts";

/**
 * The four faces a fit needs. `safeArea` is optional because the studio's pipeline hands over
 * a view that has one, while a face a test or a host builds need not: a view that says
 * nothing has no chrome over it, which is the fit this file had before ST-4.
 */
export type FitFace = Pick<View, "frame" | "setCamera" | "viewport" | "limits">
  & { readonly safeArea?: () => FitArea | null };

function countOf(mask: Uint8Array | null, nodeCount: number): number {
  if (mask === null) return nodeCount;
  let kept = 0;
  for (let i = 0; i < mask.length; i += 1) if (mask[i] === 1) kept += 1;
  return kept;
}

/**
 * A box of numbers. `camera.ts`'s `clamp` propagates a NaN rather than refusing one, so a
 * single non-finite coordinate would reach the canvas as a NaN camera, which nothing undoes.
 */
function isFiniteBounds(bounds: Bounds): boolean {
  return Number.isFinite(bounds.minX) && Number.isFinite(bounds.minY)
    && Number.isFinite(bounds.maxX) && Number.isFinite(bounds.maxY);
}

export function fitResults(view: FitFace, state: StudioState): Outcome {
  const { meta, settings } = state;
  if (meta === null) return { message: "nothing is drawn to fit" };
  const mask = highlightOf(meta, settings.filter), frame = view.frame();
  const kept = mask ?? new Uint8Array(frame.nodeCount).fill(1);
  const span = boundsOfVisible(frame, kept, frame.r ?? new Float32Array(frame.nodeCount));
  // Ponytail: an empty result set has no box, and a camera left where it was reads as a
  // button that did nothing, so the first node stands in. Escape hatch: `fit` fits all.
  const bounds = span ?? (frame.nodeCount > 0 ? { minX: 0, minY: 0, maxX: 1, maxY: 1 } : null);
  if (bounds === null) return { message: "the drawing holds no nodes" };
  if (!isFiniteBounds(bounds)) return { message: "the drawing holds a coordinate that is not a number" };
  // The renderer's own fit, not a second one: it centres in the safe area ST-4 measured,
  // takes the look's slack as a margin, keeps its "never past 2" ceiling and its degenerate
  // frame floor. Naming `maxScale` here lifted that ceiling to the view's own 40 and two
  // close results came out at 29×, which reads as a broken view; the ceiling is the renderer's.
  const area = view.safeArea?.() ?? null;
  view.setCamera(fitCamera(bounds, view.viewport(), { margin: FIT_MARGIN, area: area ?? undefined }));
  return { message: `fitted to ${countOf(mask, frame.nodeCount)} of ${frame.nodeCount} nodes` };
}
