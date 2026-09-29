/** Pointer and wheel on the canvas, turned into camera and pick requests. */
import type { Point } from "./camera.ts";
import { type DragKind, dragKindOf, isClick, isDrag, travelledBy, wheelFactor, wheelPixels } from "./gesture.ts";
import * as space from "./spacebar.ts";

export interface PointerHandlers {
  zoom(at: Point, factor: number): void;
  pan(delta: Point): void;
  /** `null` when the pointer leaves the canvas. */
  hover(at: Point | null): void;
  click(at: Point): void;
  /** The two clicks of a double-click, on the background. */
  doubleClick(at: Point): void;
}

/** A wheel in line mode reports lines, not pixels; the source's 16 px to a line. */
const LINE_HEIGHT = 16;

interface Drag {
  readonly pointer: number;
  readonly kind: DragKind;
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
  const pixels = wheelPixels(event.deltaY, event.deltaMode, LINE_HEIGHT);
  handlers.zoom(localPoint(canvas, event), wheelFactor(pixels, event.ctrlKey));
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
  drag.travelled = travelledBy(dx, dy, drag.travelled);
  // A drag that selects a node still pans the camera: the node follows the pointer in S2.
  if (isDrag(drag.travelled) || drag.kind === "pan") handlers.pan({ x: dx, y: dy });
}

function beginDrag(event: PointerEvent): Drag | null {
  const kind = dragKindOf(event.button, space.isDown());
  if (kind === "none") return null;
  // The middle button pastes and autoscrolls where the browser feels like it.
  if (event.button !== 0) event.preventDefault();
  return { pointer: event.pointerId, kind, lastX: event.clientX, lastY: event.clientY, travelled: 0 };
}

/** Space is read from the window: a key held while the pointer is captured is still on it. */
function bindSpace(target: Pick<Window, "addEventListener" | "removeEventListener">, signal: AbortSignal): void {
  const options = { signal };
  target.addEventListener("keydown", (event) => space.press(event), options);
  target.addEventListener("keyup", (event) => space.release(event), options);
  target.addEventListener("blur", () => space.forget(), options);
}

export function bindPointer(
  canvas: HTMLCanvasElement,
  handlers: PointerHandlers,
  owner: Pick<Window, "addEventListener" | "removeEventListener"> = globalThis.window,
): () => void {
  const stop = new AbortController();
  const options = { signal: stop.signal };
  let drag: Drag | null = null;
  canvas.addEventListener("wheel", (event) => onWheel(canvas, handlers, event), { ...options, passive: false });
  canvas.addEventListener("pointerdown", (event) => {
    // A second finger on a touchscreen is not a second drag: the first one owns the camera.
    if (drag !== null) return;
    drag = beginDrag(event);
    if (drag !== null) canvas.setPointerCapture(event.pointerId);
  }, options);
  canvas.addEventListener("pointermove", (event) => onMove(canvas, handlers, { event, drag }), options);
  canvas.addEventListener("pointerup", (event) => {
    const ended = drag;
    drag = null;
    if (ended === null) return;
    if (isClick(ended.travelled) && ended.kind === "select") handlers.click(localPoint(canvas, event));
  }, options);
  canvas.addEventListener("pointercancel", () => { drag = null; }, options);
  canvas.addEventListener("pointerleave", () => handlers.hover(null), options);
  canvas.addEventListener("dblclick", (event) => handlers.doubleClick(localPoint(canvas, event)), options);
  canvas.addEventListener("contextmenu", (event) => event.preventDefault(), options);
  bindSpace(owner, stop.signal);
  return () => {
    stop.abort();
    space.forget();
  };
}
