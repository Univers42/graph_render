/**
 * What a press does with the gesture it was handed, read without a browser.
 *
 * The regression these cover: a press that never travels takes the click path, which is the
 * only exit it takes, so a gesture that reserved something on the press kept that reservation
 * for the rest of the session and every later drag on it was silently refused.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { bindPointer, type Gesture, type PointerHandlers } from "../src/pointer.ts";

interface Log { readonly entries: string[] }

interface Rig {
  readonly log: Log;
  readonly canvas: HTMLCanvasElement;
  readonly down: (at: { x: number; y: number }) => void;
  readonly move: (at: { x: number; y: number }) => void;
  readonly up: (at: { x: number; y: number }) => void;
}

/** A gesture that records the three ways a press can be finished. */
function gesture(log: Log, name: string): Gesture {
  return {
    move: () => log.entries.push(`${name}.move`),
    end: () => log.entries.push(`${name}.end`),
    cancel: () => log.entries.push(`${name}.cancel`),
  };
}

function rig(name = "g"): Rig {
  const log: Log = { entries: [] };
  const canvas = { getBoundingClientRect: () => ({ left: 0, top: 0 }), addEventListener() {}, setPointerCapture() {} };
  const handlers: PointerHandlers = {
    zoom: () => undefined,
    pan: () => undefined,
    orbit: () => undefined,
    hover: () => undefined,
    click: () => log.entries.push("click"),
    press: () => gesture(log, name),
    context: () => undefined,
    doubleClick: () => undefined,
  };
  const owner = { addEventListener() {}, removeEventListener() {} };
  const stop = bindPointer(canvas as unknown as HTMLCanvasElement, handlers, owner as unknown as Window);
  const at = { clientX: 0, clientY: 0, pointerId: 1, button: 0, buttons: 1, shiftKey: false };
  // The harness's own listeners are stubs, so the events go through `press` by hand: the
  // binding is what is under test, and it reads `press` on the way in and `end` on the way out.
  stop();
  return {
    log,
    canvas: canvas as unknown as HTMLCanvasElement,
    down: () => log.entries.push("press"),
    move: () => undefined,
    up: () => undefined,
  };
}

test("a click cancels the gesture the press handed out", () => {
  const log: Log = { entries: [] };
  const canvas = document.createElement("canvas");
  const handlers: PointerHandlers = {
    zoom: () => undefined, pan: () => undefined, orbit: () => undefined, hover: () => undefined,
    click: () => log.entries.push("click"),
    press: () => gesture(log, "g"),
    context: () => undefined, doubleClick: () => undefined,
  };
  const stop = bindPointer(canvas, handlers, window);
  const fire = (type: string, x: number): void => {
    canvas.dispatchEvent(new PointerEvent(type, { clientX: x, clientY: 0, pointerId: 1, button: 0, buttons: type === "pointerup" ? 0 : 1, bubbles: true }));
  };
  fire("pointerdown", 0);
  fire("pointerup", 0);
  stop();
  assert.deepEqual(log.entries, ["g.cancel", "click"], "a click takes cancel, never end");
});

test("a drag that travelled ends the gesture instead", () => {
  const log: Log = { entries: [] };
  const canvas = document.createElement("canvas");
  const handlers: PointerHandlers = {
    zoom: () => undefined, pan: () => undefined, orbit: () => undefined, hover: () => undefined,
    click: () => log.entries.push("click"),
    press: () => gesture(log, "g"),
    context: () => undefined, doubleClick: () => undefined,
  };
  const stop = bindPointer(canvas, handlers, window);
  const fire = (type: string, x: number, buttons: number): void => {
    canvas.dispatchEvent(new PointerEvent(type, { clientX: x, clientY: 0, pointerId: 1, button: 0, buttons, bubbles: true }));
  };
  fire("pointerdown", 0, 1);
  fire("pointermove", 60, 1);
  fire("pointerup", 60, 0);
  stop();
  assert.deepEqual(log.entries, ["g.move", "g.end"], "a drag that moved is the gesture's, and the click does not fire");
});