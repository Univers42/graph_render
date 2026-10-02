/** Pointer and wheel on the canvas, turned into camera and pick requests. */
import type { Point } from "./camera.ts";
import { RIGHT_BUTTON, type DragKind, dragKindOf, isClick, isDrag, travelledBy, wheelFactor, wheelPixels } from "./gesture.ts";
import * as space from "./spacebar.ts";

export interface PointerHandlers {
  zoom(at: Point, factor: number): void;
  pan(delta: Point): void;
  /**
   * A 3D frame's background drag. `right` is the right button, which slides the target
   * rather than turning the camera; both are the same gesture with a different intent.
   * Never called for a 2D frame, so the 2D path cannot reach an orbit by accident.
   */
  orbit(delta: Point, right: boolean): void;
  /** `null` when the pointer leaves the canvas. */
  hover(at: Point | null): void;
  click(at: Point, shift: boolean): void;
  /**
   * A left press on the canvas. A gesture takes the drag (a node, or a box with shift held);
   * `null` leaves it to the camera.
   */
  press(at: Point, shift: boolean): Gesture | null;
  /** The secondary button, or the menu key's stand-in: where, on the canvas. */
  context(at: Point): void;
  /** The two clicks of a double-click, on the background. */
  doubleClick(at: Point): void;
}

/** What a drag does when it is not a pan: told where the pointer is, and where it let go. */
export interface Gesture {
  move(at: Point): void;
  end(at: Point): void;
  /** The pointer was lost (cancel, window blur): undo what `move` did. */
  cancel?(): void;
}

/** A wheel in line mode reports lines, not pixels; the source's 16 px to a line. */
const LINE_HEIGHT = 16;

interface Drag {
  readonly pointer: number;
  readonly kind: DragKind;
  readonly gesture: Gesture | null;
  /** True when the orbit came from the right button, which pans rather than turns. */
  readonly right: boolean;
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
  if (drag.gesture !== null) {
    if (isDrag(drag.travelled)) drag.gesture.move(localPoint(canvas, event));
  } else if (drag.kind === "orbit") {
    // No slop for the orbit: the gesture is the camera, and a slop would make the first few
    // pixels of every drag a dead zone the 2D pan does not have.
    handlers.orbit({ x: dx, y: dy }, drag.right);
  } else if (isDrag(drag.travelled) || drag.kind === "pan") handlers.pan({ x: dx, y: dy });
}

function beginDrag(event: PointerEvent, gesture: Gesture | null, inSpace: boolean): Drag | null {
  const kind = dragKindOf(event.button, space.isDown(), inSpace);
  if (kind === "none") return null;
  // The middle button pastes and autoscrolls where the browser feels like it.
  if (event.button !== 0) event.preventDefault();
  return {
    pointer: event.pointerId, kind, gesture, right: event.button === RIGHT_BUTTON,
    lastX: event.clientX, lastY: event.clientY, travelled: 0,
  };
}

/** Space is read from the window: a key held while the pointer is captured is still on it. */
function bindSpace(target: Pick<Window, "addEventListener" | "removeEventListener">, signal: AbortSignal): void {
  const options = { signal };
  target.addEventListener("keydown", (event) => space.press(event), options);
  target.addEventListener("keyup", (event) => space.release(event), options);
  target.addEventListener("blur", () => space.forget(), options);
}

function finishDrag(canvas: HTMLCanvasElement, handlers: PointerHandlers, done: { ended: Drag; event: PointerEvent }): void {
  const { ended, event } = done;
  if (ended.kind === "orbit") {
    // A 3D press that never moved is a click on a node, not a camera that did nothing: the
    // drag owns the pointer, the click still selects. A drag that did move is the camera.
    if (isClick(ended.travelled) && !ended.right) handlers.click(localPoint(canvas, event), event.shiftKey);
    return;
  }
  if (ended.kind !== "select") return;
  if (isClick(ended.travelled)) handlers.click(localPoint(canvas, event), event.shiftKey);
  else ended.gesture?.end(localPoint(canvas, event));
}

/**
 * One press, started. A second finger on a touchscreen is not a second drag: the first one
 * owns the camera. On a 3D frame a left press never grabs a node, because a drag there is
 * the camera and the press is only a click waiting to be a click; a 2D frame grabs a node
 * exactly as it always did.
 */
function onDown(press: { canvas: HTMLCanvasElement; handlers: PointerHandlers; inSpace: () => boolean }, event: PointerEvent, drag: Drag | null): Drag | null {
  if (drag !== null) return null;
  const space3d = press.inSpace();
  const grab = event.button === 0 && !space.isDown() && !space3d
    ? press.handlers.press(localPoint(press.canvas, event), event.shiftKey)
    : null;
  return beginDrag(event, grab, space3d);
}

export function bindPointer(
  canvas: HTMLCanvasElement,
  handlers: PointerHandlers,
  owner: Pick<Window, "addEventListener" | "removeEventListener"> = globalThis.window,
  /** Whether the frame on screen is 3D, asked per press: a layout can change under the hand. */
  inSpace: () => boolean = () => false,
): () => void {
  const stop = new AbortController();
  const options = { signal: stop.signal };
  let drag: Drag | null = null;
  canvas.addEventListener("wheel", (event) => onWheel(canvas, handlers, event), { ...options, passive: false });
  canvas.addEventListener("pointerdown", (event) => {
    drag = onDown({ canvas, handlers, inSpace }, event, drag);
    if (drag !== null) canvas.setPointerCapture(event.pointerId);
  }, options);
  canvas.addEventListener("pointermove", (event) => onMove(canvas, handlers, { event, drag }), options);
  canvas.addEventListener("pointerup", (event) => {
    const ended = drag;
    drag = null;
    if (ended !== null) finishDrag(canvas, handlers, { ended, event });
  }, options);
  const abandon = (): void => {
    drag?.gesture?.cancel?.();
    drag = null;
  };
  canvas.addEventListener("pointercancel", abandon, options);
  owner.addEventListener("blur", abandon, options);
  canvas.addEventListener("pointerleave", () => handlers.hover(null), options);
  canvas.addEventListener("dblclick", (event) => handlers.doubleClick(localPoint(canvas, event)), options);
  canvas.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    // On a 3D frame the right button is the camera's, so the node menu would sit on top of
    // a gesture the user is still making. It stays exactly where it was on a 2D frame.
    if (inSpace()) return;
    handlers.context(localPoint(canvas, event));
  }, options);
  bindSpace(owner, stop.signal);
  return () => {
    stop.abort();
    space.forget();
  };
}
