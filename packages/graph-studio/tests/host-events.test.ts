/**
 * Row `host-api-unit`, the events (`docs/contract/host-api.md`, Events; verdict 10): what one
 * event carries, and how often the watcher sends each one. Whether a `document` listener hears an
 * event from inside a shadow root is the browser's to say, so `composed` is asserted in
 * `scripts/studio-embed.sh`, never against a fake DOM here.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { ViewEvents } from "../../graph-render/src/view.ts";
import type { HostEvents } from "../src/host/contract.ts";
import { emit } from "../src/host/events.ts";
import { createPreviews } from "../src/host/previews.ts";
import { watchHost } from "../src/host/watch.ts";
import type { ShownError } from "../src/state/errors.ts";
import { type StudioState, initialState } from "../src/state/model.ts";
import { createStore } from "../src/state/store.ts";
import type { FrameScheduler } from "../src/ui/frameThrottle.ts";
import { DRAWN, META } from "./drawn.ts";

function deepFrozen(value: unknown): boolean {
  if (typeof value !== "object" || value === null) return true;
  return Object.isFrozen(value) && Object.values(value).every(deepFrozen);
}

/** What verdict 10 asks of one event, short of `composed` reaching the page (the browser gate). */
function wellFormed(event: Event, sent: unknown): boolean {
  return event instanceof CustomEvent && event.bubbles && event.composed && deepFrozen(event.detail) && event.detail !== sent;
}

function caught(send: (target: EventTarget, detail: HostEvents["node-select"]) => void): boolean {
  const target = new EventTarget();
  const detail = { ids: ["a", "b"] };
  let held: Event | null = null;
  target.addEventListener("node-select", (event) => (held = event));
  send(target, detail);
  detail.ids.push("c");
  return held !== null && wellFormed(held, detail);
}

test("emit sends a bubbling, composed event whose detail is a frozen copy", () => {
  assert.equal(caught((target, detail) => emit(target, "node-select", detail)), true);
});

test("negative control: a plain CustomEvent with the caller's own object fails the same check", () => {
  assert.equal(caught((target, detail) => target.dispatchEvent(new CustomEvent("node-select", { detail }))), false);
});

/** Frames that run when the test says, or at once when `immediate`. */
function frames(immediate: boolean): FrameScheduler & { readonly flush: () => void } {
  const pending: (() => void)[] = [];
  return {
    next: (run) => {
      if (immediate) run();
      else pending.push(run);
      return () => void pending.splice(pending.indexOf(run), 1);
    },
    flush: () => {
      for (const run of pending.splice(0)) run();
    },
  };
}

interface Watched {
  readonly heard: string[];
  readonly store: ReturnType<typeof createStore<StudioState>>;
  readonly hover: (node: number) => void;
}

function watched(state: StudioState, scheduler: FrameScheduler): Watched {
  const store = createStore(state);
  const host = new EventTarget();
  const heard: string[] = [];
  for (const name of ["graph-load", "node-select", "graph-error", "node-hover"]) {
    host.addEventListener(name, (event) => {
      if (event instanceof CustomEvent) heard.push(`${name} ${JSON.stringify(event.detail)}`);
    });
  }
  const hovers = new Set<(node: ViewEvents["hover"]) => void>();
  const view = {
    on: <Name extends keyof ViewEvents>(name: Name, handler: (payload: ViewEvents[Name]) => void): (() => void) => {
      if (name !== "hover") return () => undefined;
      const typed = (node: number): void => handler(node);
      hovers.add(typed);
      return () => void hovers.delete(typed);
    },
  };
  watchHost({ host, store, view, previews: createPreviews({ resolver: () => null }), frames: scheduler });
  return { heard, store, hover: (node) => hovers.forEach((handler) => handler(node)) };
}

function hoversAcross(immediate: boolean): string[] {
  const scheduler = frames(immediate);
  const subject = watched(DRAWN, scheduler);
  for (const node of [0, 1, 2]) subject.hover(node);
  scheduler.flush();
  return subject.heard;
}

test("node-hover is sent at most once a frame, for the node the pointer ended on", () => {
  assert.deepEqual(hoversAcross(false), ['node-hover {"id":"c"}']);
});

test("negative control: with a frame per pointer move the same crossing is three events", () => {
  assert.equal(hoversAcross(true).length, 3);
});

test("graph-load waits for the new graph's frame, then is sent once", () => {
  const subject = watched(DRAWN, frames(false));
  const graph = { name: "next", nodeCount: 3, edgeCount: 1, notes: ["n1", "n2"], buildMs: 1 };
  subject.store.set({ ...DRAWN, graph });
  assert.deepEqual(subject.heard, [], "the old frame is still on screen");
  subject.store.set({ ...DRAWN, graph, meta: { ...META } });
  subject.store.set({ ...DRAWN, graph, meta: { ...META } });
  assert.deepEqual(subject.heard, ['graph-load {"nodes":3,"edges":1,"notes":["n1","n2"]}']);
});

const REFUSED: ShownError = { title: "RunRefusedError", code: "LayoutRefused", detail: "the layout refused", hint: "" };

test("graph-error is sent once per error, named by its code, else by its title", () => {
  const subject = watched(initialState(), frames(false));
  subject.store.set({ ...initialState(), error: REFUSED });
  subject.store.set({ ...subject.store.get(), selection: [] });
  subject.store.set({ ...initialState(), error: { ...REFUSED, code: null } });
  assert.deepEqual(subject.heard, [
    'graph-error {"error":"LayoutRefused","message":"the layout refused"}',
    'graph-error {"error":"RunRefusedError","message":"the layout refused"}',
  ]);
});
