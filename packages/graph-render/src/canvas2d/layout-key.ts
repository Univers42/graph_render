/**
 * When the label layout has to run: only when something it reads has changed. A redraw for
 * a fade, a selection ring or a marquee reads the same plan again.
 *
 * Ponytail: the key compares the scene, theme, policy and position array by identity, so a
 * host that mutates one in place without replacing it is not seen; the view's own writers
 * (drag) raise `layoutDirty` through `markMoved`. A change of hover or selection that
 * keeps the same focused node is not a layout change, and lays nothing out.
 */
import type { Camera, Viewport } from "../camera.ts";

export interface LayoutKey {
  readonly scene: object;
  readonly theme: object;
  readonly policy: object;
  readonly x: object;
  readonly camera: Camera;
  readonly viewport: Viewport;
  readonly dpr: number;
  readonly baked: number;
  readonly focus: number;
}

export interface Keyed {
  readonly scene: object;
  readonly theme: object;
  readonly policy: object;
  readonly x: object;
  readonly camera: Camera;
  readonly viewport: Viewport;
  readonly dpr: number;
  readonly sprites: { baked(): number };
  layoutKey: LayoutKey | null;
  layoutDirty: boolean;
}

function same(a: LayoutKey, b: LayoutKey): boolean {
  return a.scene === b.scene && a.theme === b.theme && a.policy === b.policy && a.x === b.x
    && a.camera.x === b.camera.x && a.camera.y === b.camera.y && a.camera.scale === b.camera.scale
    && a.viewport.width === b.viewport.width && a.viewport.height === b.viewport.height
    && a.dpr === b.dpr && a.baked === b.baked && a.focus === b.focus;
}

/** True when the layout must run for this frame; remembers what it ran for. */
export function layoutChanged(state: Keyed, focus: number, travelling: boolean): boolean {
  const key: LayoutKey = {
    scene: state.scene, theme: state.theme, policy: state.policy, x: state.x, camera: state.camera,
    viewport: state.viewport, dpr: state.dpr, baked: state.sprites.baked(), focus,
  };
  const previous = state.layoutKey;
  state.layoutKey = key;
  const dirty = state.layoutDirty;
  state.layoutDirty = false;
  return travelling || dirty || previous === null || !same(previous, key);
}
