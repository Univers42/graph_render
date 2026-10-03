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
import type { Point } from "../src/camera.ts";

/** Just enough of a canvas for `bindPointer`: listeners, a box, and pointer capture. */
class FakeCanvas {
  private readonly listeners = new Map<string, ((event: unknown) => void)[]>();

  addEventListener(type: string, handler: (event: unknown) => void): void {
    const held = this.listeners.get(type) ?? [];
    held.push(handler);
    this.listeners.set(type, held);
  }

  getBoundingClientRect(): { left: number; top: number } {
    return { left: 0, top: 0 };
  }

  setPointerCapture(): void { /* nothing is captured in a test */ }

  fire(type: string, event: unknown): void {
    for (const handler of this.listeners.get(type) ?? []) handler(event);
  }
}

interface Press { clientX: number; clientY: number; pointerId: number; button: number; buttons: number; shiftKey: boolean }

function pointer(x: number, buttons: number): Press {
  return { clientX: x, clientY: 0, pointerId: 1, button: 0, buttons, shiftKey: false };
}

/** A gesture that records the three ways a press can be finished. */
function recorder(entries: string[]): Gesture {
  return {
    move: () => entries.push("move"),
    end: () => entries.push("end"),
    cancel: () => entries.push("cancel"),
  };
}

function rig(): { entries: string[]; canvas: FakeCanvas; stop: () => void } {
  const entries: string[] = [];
  const canvas = new FakeCanvas();
  const owner = { addEventListener() {}, removeEventListener() {} };
  const handlers: PointerHandlers = {
    zoom: () => undefined,
    pan: () => undefined,
    orbit: () => undefined,
    hover: () => undefined,
    click: () => entries.push("click"),
    press: (_at: Point) => recorder(entries),
    context: () => undefined,
    doubleClick: () => undefined,
  };
  const stop = bindPointer(canvas as unknown as HTMLCanvasElement, handlers, owner as unknown as Window);
  return { entries, canvas, stop };
}

test("a click takes cancel, never end: it is the only exit a click gets", () => {
  const { entries, canvas, stop } = rig();
  canvas.fire("pointerdown", pointer(0, 1));
  canvas.fire("pointerup", pointer(0, 0));
  stop();
  assert.deepEqual(entries, ["cancel", "click"]);
});

test("a press that travelled ends the gesture, and the click does not fire", () => {
  const { entries, canvas, stop } = rig();
  canvas.fire("pointerdown", pointer(0, 1));
  canvas.fire("pointermove", pointer(60, 1));
  canvas.fire("pointerup", pointer(60, 0));
  stop();
  assert.deepEqual(entries, ["move", "end"]);
});

test("a cancelled press — the pointer lost — cancels rather than ends", () => {
  const { entries, canvas, stop } = rig();
  canvas.fire("pointerdown", pointer(0, 1));
  canvas.fire("pointercancel", pointer(0, 1));
  stop();
  assert.deepEqual(entries, ["cancel"]);
});