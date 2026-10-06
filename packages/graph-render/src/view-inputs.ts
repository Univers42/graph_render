/** What a view binds to its canvas — pointer, wheel and resize — and how it encodes the canvas as PNG. Out of view.ts for its 300 lines. */
import { type Point, panBy, zoomAt } from "./camera.ts";
import { inSpace, orbitBy, zoomAt3d } from "./camera-api.ts";
import { clickAt, contextAt, pressAt } from "./canvas2d/choose.ts";
import { type Controller, fit, hover, measure, moveTo, pickAt } from "./canvas2d/controller.ts";
import { invalidate } from "./canvas2d/loop.ts";
import { DOUBLE_CLICK_ZOOM } from "./gesture.ts";
import { taken } from "./gestured.ts";
import { bindPointer } from "./pointer.ts";

export function toBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    canvas.toBlob((blob) => {
      if (blob === null) reject(new Error("graph-render: the canvas could not be encoded as PNG"));
      else resolve(blob);
    }, "image/png");
  });
}

/** Pointer, wheel and resize; returns what undoes all three. */
export function bindInputs(controller: Controller): () => void {
  const { canvas, state } = controller;
  // Every gesture the pointer layer reports, not the ones that happen to move the camera: a click
  // and a node drag leave the camera where it is, and both still mean the user has taken it over
  // from the view's automatic fit (`gestured.ts`).
  const unbind = bindPointer(canvas, {
    zoom: taken(controller, (at: Point, factor: number) => {
      if (state.orbit !== null) zoomAt3d(controller, factor);
      else moveTo(controller, zoomAt(state.camera, at, factor, state.limits), false);
    }),
    pan: taken(controller, (delta: Point) => moveTo(controller, panBy(state.camera, delta), false)),
    orbit: taken(controller, (delta: Point, right: boolean) => orbitBy(controller, delta, right)),
    hover: (at) => hover(controller, at === null ? -1 : pickAt(state, at)),
    click: taken(controller, (at: Point, shift: boolean) => clickAt(controller, at, shift)),
    press: taken(controller, (at: Point, shift: boolean) => pressAt(controller, at, shift)),
    context: taken(controller, (at: Point) => contextAt(controller, at)),
    doubleClick: taken(controller, (at: Point) => {
      // A double-click on a node is the node's own gesture (S2); on the background it zooms.
      if (pickAt(state, at) >= 0) return;
      if (state.orbit !== null) zoomAt3d(controller, DOUBLE_CLICK_ZOOM);
      else moveTo(controller, zoomAt(state.camera, at, DOUBLE_CLICK_ZOOM, state.limits), false);
    }),
  }, globalThis.window, () => inSpace(state));
  const observer = new ResizeObserver(() => {
    measure(controller);
    if (controller.fitted) fit(controller);
    else invalidate(state);
  });
  observer.observe(canvas);
  canvas.style.cursor = "grab";
  canvas.style.touchAction = "none";
  return () => {
    observer.disconnect();
    unbind();
  };
}
