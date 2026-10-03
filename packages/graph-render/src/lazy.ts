/**
 * A scene whose derived state is computed when it is read, not when it is handed over.
 *
 * A drag rebuilds the scene once per pointer move, so every O(n) pass the scene carries
 * (`bounds`, `grid`) is paid on each frame while nothing reads it: the grid is only
 * consulted by a pick, and the bounds only by a fit. Here both are accessors, computed on
 * the first read of a given scene and cached after that.
 *
 * Because `Scene.grid` and `Scene.bounds` are data properties on the interface, the scene
 * below is built field by field and never spread: spreading it would fire every getter and
 * buy back the eager cost the accessors exist to avoid.
 */
import type { Bounds } from "./camera.ts";
import type { Frame } from "./frame.ts";
import { type Grid, type Positions, gridOf } from "./grid.ts";
import { type Scene, boundsOf } from "./scene.ts";

/**
 * The scene with these columns as the node positions, and the bounds and grid of those
 * columns read on demand. The first reader pays one O(n) pass; a frame nobody picks in and
 * nobody fits pays none.
 */
export function deferredScene(scene: Scene, positions: Positions): Scene {
  const frame: Frame = { ...scene.frame, x: positions.x, y: positions.y };
  let bounds: Bounds | null = null;
  let grid!: Grid;
  let boundsRead = false;
  let gridRead = false;
  const readBounds = (): Bounds | null => {
    if (!boundsRead) {
      bounds = boundsOf(positions, scene.reach);
      boundsRead = true;
    }
    return bounds;
  };
  const readGrid = (): Grid => {
    if (!gridRead) {
      // The grid is shaped over the bounds of these columns, so the scan is shared.
      grid = gridOf(positions, readBounds());
      gridRead = true;
    }
    return grid;
  };
  return {
    frame,
    style: scene.style,
    adjacency: scene.adjacency,
    extent: scene.extent,
    reach: scene.reach,
    get bounds() {
      return readBounds();
    },
    get grid() {
      return readGrid();
    },
  };
}
