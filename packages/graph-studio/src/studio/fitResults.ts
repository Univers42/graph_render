/** Frames the camera on the nodes the search highlighted, with the look's slack around them. */
import { type Bounds, type Camera, type Viewport, type ZoomLimits, clamp } from "../../../graph-render/src/camera.ts";
import { boundsOfVisible } from "../../../graph-render/src/local.ts";
import { FIT_MARGIN } from "../../../graph-render/src/look/presets.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Outcome } from "../actions/registry.ts";
import { highlightOf } from "../look/visibleOf.ts";
import type { StudioState } from "../state/model.ts";

export type FitFace = Pick<View, "frame" | "setCamera" | "viewport" | "limits">;

function countOf(mask: Uint8Array | null, nodeCount: number): number {
  if (mask === null) return nodeCount;
  let kept = 0;
  for (let i = 0; i < mask.length; i += 1) if (mask[i] === 1) kept += 1;
  return kept;
}

/** The camera that puts a box inside the viewport with the look's slack, within the limits. */
function cameraFor(bounds: Bounds, viewport: Viewport, limits: ZoomLimits): Camera {
  const width = Math.max(1, bounds.maxX - bounds.minX), height = Math.max(1, bounds.maxY - bounds.minY);
  const room = Math.min(viewport.width, viewport.height) / FIT_MARGIN;
  const scale = clamp(Math.min(room / width, room / height), limits.min, limits.max);
  const midX = (bounds.minX + bounds.maxX) / 2, midY = (bounds.minY + bounds.maxY) / 2;
  return { scale, x: viewport.width / 2 - midX * scale, y: viewport.height / 2 - midY * scale };
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
  view.setCamera(cameraFor(bounds, view.viewport(), view.limits()));
  return { message: `fitted to ${countOf(mask, frame.nodeCount)} of ${frame.nodeCount} nodes` };
}
