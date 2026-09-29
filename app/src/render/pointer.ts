/**
 * Pointer wiring for a graph panel: drag to pan, wheel to zoom at the cursor,
 * hover to hit-test, a click (under 4 px of travel) to select, double-click to
 * fit. Kept out of `GraphView` so that class stays about the canvas and the
 * camera rather than about event plumbing.
 */

import { panBy, zoomAt } from "../../../src/core/camera/controls.ts";
import type { Camera } from "../../../src/core/camera/transform.ts";
import type { Point } from "../core/hitTest.ts";

/** Screen pixels of grab slack around a node, so small nodes stay clickable. */
export const HIT_TOLERANCE_PX = 4;
/** Pointer travel under this is a click; over it is a pan. */
const CLICK_SLOP_PX = 4;

export interface PointerHost {
  /** The canvas the listeners attach to. */
  canvas(): HTMLCanvasElement;
  camera(): Camera;
  setCamera(camera: Camera): void;
  /** Dense index under a canvas-local point, or -1. */
  hitAt(point: Point): number;
  fit(): void;
  onHover(index: number, at: Point | null): void;
  onSelect(index: number): void;
}

interface Handlers {
  readonly down: (event: PointerEvent) => void;
  readonly move: (event: PointerEvent) => void;
  readonly up: (event: PointerEvent) => void;
  readonly leave: () => void;
  readonly wheel: (event: WheelEvent) => void;
  readonly double: () => void;
}

/** The six handlers, sharing the "is a drag in progress, and how far has it
 *  travelled" state that decides click from pan. */
function makeHandlers(host: PointerHost, canvas: HTMLCanvasElement): Handlers {
  let panning = false;
  let travelled = 0;
  const local = (event: { clientX: number; clientY: number }): Point => {
    const rect = canvas.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };
  return {
    down: (event) => {
      panning = true;
      travelled = 0;
      canvas.setPointerCapture(event.pointerId);
    },
    move: (event) => {
      const point = local(event);
      if (!panning) {
        host.onHover(host.hitAt(point), point);
        return;
      }
      travelled += Math.abs(event.movementX) + Math.abs(event.movementY);
      host.setCamera(panBy(host.camera(), event.movementX, event.movementY));
    },
    up: (event) => {
      panning = false;
      if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
      if (travelled <= CLICK_SLOP_PX) host.onSelect(host.hitAt(local(event)));
    },
    leave: () => host.onHover(-1, null),
    wheel: (event) => {
      event.preventDefault();
      const point = local(event);
      host.setCamera(zoomAt(host.camera(), point.x, point.y, Math.exp(-event.deltaY * 0.0016)));
    },
    double: () => host.fit(),
  };
}

/** Attach every pointer listener to `host`'s canvas. Returns the detach fn. */
export function bindPointer(host: PointerHost): () => void {
  const canvas = host.canvas();
  const h = makeHandlers(host, canvas);
  canvas.addEventListener("pointerdown", h.down);
  canvas.addEventListener("pointermove", h.move);
  canvas.addEventListener("pointerleave", h.leave);
  canvas.addEventListener("wheel", h.wheel, { passive: false });
  canvas.addEventListener("dblclick", h.double);
  globalThis.addEventListener("pointerup", h.up);

  return () => {
    canvas.removeEventListener("pointerdown", h.down);
    canvas.removeEventListener("pointermove", h.move);
    canvas.removeEventListener("pointerleave", h.leave);
    canvas.removeEventListener("wheel", h.wheel);
    canvas.removeEventListener("dblclick", h.double);
    globalThis.removeEventListener("pointerup", h.up);
  };
}
