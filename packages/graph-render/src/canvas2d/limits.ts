/**
 * The zoom limits, and the safe area they are fitted into.
 *
 * WHY this is a module and not two lines in the controller: `limitsFor` reads the scene's
 * bounds, and the bounds of a live frame are an O(n) scan over every node. Nothing reads the
 * limits between two zoom gestures — `view.ts` and `camera-api.ts` read them on a wheel, a
 * double click and a zoom call, and nothing else does — so a settle that rebuilt them every
 * animation frame paid for a million-node scan sixty times a second to keep a number no hand
 * had asked for. The limits are therefore built when they are read.
 *
 * Ponytail: the cache is keyed on what the limits actually depend on — the scene, the
 * viewport box and the safe area — and nothing else, so a frame, a resize and a host
 * reporting its chrome each invalidate it without an explicit "dirty" flag that could be
 * forgotten. Failing input: a scene whose bounds change while its identity does not (nothing
 * does today — `setPositions` and `movedScene` both install a new scene object) would serve a
 * stale limit. Direction: identity, because every mutation of the drawing goes through a new
 * scene. Escape hatch: `held.delete(state)` forces the next read to rebuild.
 */
import { type FitArea, type ZoomLimits, limitsFor } from "../camera.ts";
import { safeOf } from "../gestured.ts";
import type { LoopState } from "./loop.ts";

interface Held {
  readonly scene: unknown;
  readonly width: number;
  readonly height: number;
  readonly safe: FitArea | null;
  readonly value: ZoomLimits;
}

const held = new WeakMap<LoopState, Held>();

/** The limits for the drawing as it stands now, rebuilt only when what they depend on moved. */
export function currentLimits(state: LoopState): ZoomLimits {
  const { scene, viewport, safe } = state;
  const hit = held.get(state);
  if (hit !== undefined && hit.scene === scene
    && hit.width === viewport.width && hit.height === viewport.height && hit.safe === safe) {
    return hit.value;
  }
  const value = limitsFor(scene.bounds, viewport, { area: safeOf(state, viewport) ?? undefined });
  held.set(state, { scene, width: viewport.width, height: viewport.height, safe, value });
  return value;
}
