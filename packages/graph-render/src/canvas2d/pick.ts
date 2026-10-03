/**
 * Picking: the node under a screen point, in the plane and in space. A child of
 * `controller.ts`, split out by the house's 300-line limit; `controller.ts` re-exports `pickAt`.
 */
import { type Point, screenToWorld } from "../camera.ts";
import { pickEased, pickIn } from "../scene.ts";
import type { LoopState } from "./loop.ts";
import { MIN_SCREEN_RADIUS } from "./nodes.ts";

const PICK_TOLERANCE = 4;

/**
 * The node under a screen point.
 *
 * Mid-tween this scans the eased pose rather than asking the grid, because the grid indexes
 * the *target* frame and every node is somewhere else until the tween ends. The scan reads the
 * two halves and the fraction the loop already publishes for the shader, so it is the pose
 * both backends draw; outside a tween the grid still answers, and it is O(cells) not O(nodes).
 */
export function pickAt(state: LoopState, at: Point): number {
  if (state.orbit !== null) return pickInSpace(state, at);
  const world = screenToWorld(state.camera, at);
  const { scale } = state.camera;
  const query = {
    x: world.x, y: world.y, tolerance: PICK_TOLERANCE / scale, floor: MIN_SCREEN_RADIUS / scale,
  };
  const tween = state.bulk.tween;
  return tween === null ? pickIn(state.scene, query) : pickEased(state.scene, query, tween);
}

/**
 * The 3D hit test: the nearest node whose drawn disc is under the point. There is no
 * un-projection here, and there does not need to be — the screen points are already what the
 * painter drew, so the same radii and the same tolerance pick the same node a hand would.
 * The nearest wins, so a node in front is picked over one behind it at the same point.
 */
function pickInSpace(state: LoopState, at: Point): number {
  const drawn = state.drawn;
  if (drawn === null) return -1;
  let best = -1;
  let bestDepth = Infinity;
  for (let step = 0; step < drawn.drawn; step += 1) {
    const node = drawn.order[step] ?? 0;
    if (state.scene.style.hidden?.[node] === 1) continue;
    const depth = drawn.depth[node] ?? 0;
    if (depth <= 0) continue;
    const reach = Math.max(MIN_SCREEN_RADIUS, drawn.radius[node] ?? 0) + PICK_TOLERANCE;
    const dx = (drawn.x[node] ?? 0) - at.x;
    const dy = (drawn.y[node] ?? 0) - at.y;
    if (dx * dx + dy * dy > reach * reach) continue;
    if (depth < bestDepth) {
      best = node;
      bestDepth = depth;
    }
  }
  return best;
}
