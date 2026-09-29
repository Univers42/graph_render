/**
 * Which gesture a press on the canvas is. The decisions are here, apart from the DOM, so
 * they can be read without a browser: what a button and a held space mean, and by how much
 * a wheel notch and a pinch move the scale.
 */
import type { Point } from "./camera.ts";

/** Past this many pixels a press is a drag, not a click. */
export const DRAG_SLOP = 4;
/** A double-click on the background zooms in by this much, at the cursor. */
export const DOUBLE_CLICK_ZOOM = 2;
const WHEEL_ZOOM = 0.0016;
/** A pinch arrives as a wheel with ctrlKey set and much smaller deltas. */
export const PINCH_ZOOM = 0.012;
export const MIDDLE_BUTTON = 1;

/** What a press will do if it moves: pan the camera, or drag a node. */
export type DragKind = "pan" | "select" | "none";

/**
 * WHY the space bar: a drag on a node is the node's, and the space bar is the only key a
 * hand holds while the pointer is busy. Middle-drag pans without it, for a mouse with three
 * buttons and no room for a modifier under the left one.
 */
export function dragKindOf(button: number, space: boolean): DragKind {
  if (button === MIDDLE_BUTTON) return "pan";
  if (button !== 0) return "none";
  return space ? "pan" : "select";
}

/** The scale factor a wheel notch means: a notch is `deltaY` pixels of scroll. */
export function wheelFactor(pixels: number, ctrlKey: boolean): number {
  return Math.exp(-pixels * (ctrlKey ? PINCH_ZOOM : WHEEL_ZOOM));
}

/** Line-mode wheels report lines, not pixels; the source says 16 px to a line. */
export function wheelPixels(deltaY: number, deltaMode: number, lineHeight: number): number {
  return deltaMode === 1 ? deltaY * lineHeight : deltaY;
}

/** How far a press has travelled, in pixels: the taxicab sum of what it moved. */
export function travelledBy(dx: number, dy: number, before: number): number {
  return before + Math.abs(dx) + Math.abs(dy);
}

/** True once a press has moved far enough to be a drag and not a click. */
export function isDrag(travelled: number): boolean {
  return travelled > DRAG_SLOP;
}

/** A click is a press that stayed put; a drag that ended is not a click. */
export function isClick(travelled: number): boolean {
  return travelled <= DRAG_SLOP;
}

export function centreOf(viewport: { readonly width: number; readonly height: number }): Point {
  return { x: viewport.width / 2, y: viewport.height / 2 };
}
