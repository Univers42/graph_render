/** Pointer and wheel on the canvas, turned into camera and pick requests. */
import type { Point } from "./camera.ts";

export interface PointerHandlers {
  zoom(at: Point, factor: number): void;
  pan(delta: Point): void;
  /** `null` when the pointer leaves the canvas. */
  hover(at: Point | null): void;
  click(at: Point): void;
}

/** Past this many pixels a press is a drag, not a click. */
const DRAG_SLOP = 4;
const WHEEL_ZOOM = 0.0016;
/** A pinch arrives as a wheel with ctrlKey set and much smaller deltas. */
const PINCH_ZOOM = 0.012;

interface Drag {
  readonly pointer: number;
  lastX: number;
  lastY: number;
  travelled: number;
}

function localPoint(canvas: HTMLCanvasElement, event: MouseEvent): Point {
  const box = canvas.getBoundingClientRect();
  return { x: event.clientX - box.left, y: event.clientY - box.top };
}

function onWheel(canvas: HTMLCanvasElement, handlers: PointerHandlers, event: WheelEvent): void {
  event.preventDefault();
  const pixels = event.deltaMode === WheelEvent.DOM_DELTA_LINE ? event.deltaY * 16 : event.deltaY;
  handlers.zoom(localPoint(canvas, event), Math.exp(-pixels * (event.ctrlKey ? PINCH_ZOOM : WHEEL_ZOOM)));
}

function onMove(canvas: HTMLCanvasElement, handlers: PointerHandlers, moved: { event: PointerEvent; drag: Drag | null }): void {
  const { event, drag } = moved;
  if (drag === null || drag.pointer !== event.pointerId) {
    handlers.hover(localPoint(canvas, event));
    return;
  }
  const dx = event.clientX - drag.lastX;
  const dy = event.clientY - drag.lastY;
  drag.lastX = event.clientX;
  drag.lastY = event.clientY;
  drag.travelled += Math.abs(dx) + Math.abs(dy);
  if (drag.travelled > DRAG_SLOP) handlers.pan({ x: dx, y: dy });
}

export function bindPointer(canvas: HTMLCanvasElement, handlers: PointerHandlers): () => void {
  const stop = new AbortController();
  const options = { signal: stop.signal };
  let drag: Drag | null = null;
  canvas.addEventListener("wheel", (event) => onWheel(canvas, handlers, event), { ...options, passive: false });
  canvas.addEventListener("pointerdown", (event) => {
    if (event.button !== 0) return;
    drag = { pointer: event.pointerId, lastX: event.clientX, lastY: event.clientY, travelled: 0 };
    canvas.setPointerCapture(event.pointerId);
  }, options);
  canvas.addEventListener("pointermove", (event) => onMove(canvas, handlers, { event, drag }), options);
  canvas.addEventListener("pointerup", (event) => {
    const ended = drag;
    drag = null;
    if (ended !== null && ended.travelled <= DRAG_SLOP) handlers.click(localPoint(canvas, event));
  }, options);
  canvas.addEventListener("pointercancel", () => { drag = null; }, options);
  canvas.addEventListener("pointerleave", () => handlers.hover(null), options);
  return () => stop.abort();
}
