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
import { type Store, createStore } from "../src/state/store.ts";
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
  const held: Event[] = [];
  target.addEventListener("node-select", (event) => held.push(event));
  send(target, detail);
  detail.ids.push("c");
  return held.length === 1 && held.every((event) => wellFormed(event, detail));
}

test("emit sends a bubbling, composed event whose detail is a frozen copy", () => {
  assert.equal(caught((target, detail) => emit(target, "node-select", detail)), true);
});

test("negative control: a plain CustomEvent with the caller's own object fails the same check", () => {
  assert.equal(caught((target, detail) => target.dispatchEvent(new CustomEvent("node-select", { detail }))), false);
});

/** Frames that run only when the test flushes them, as a real frame never runs inside the call. */
function frames(): FrameScheduler & { readonly flush: () => void } {
  const pending = new Set<() => void>();
  return {
    next: (run) => {
      pending.add(run);
      return () => void pending.delete(run);
    },
    flush: () => {
      const due = [...pending];
      pending.clear();
      for (const run of due) run();
    },
  };
}

interface Watched {
  readonly heard: string[];
  readonly store: Store<StudioState>;
  readonly hover: (node: number) => void;
}

type Handlers = { [Name in keyof ViewEvents]: Set<(payload: ViewEvents[Name]) => void> };

function watched(state: StudioState, scheduler: FrameScheduler): Watched {
  const store = createStore(state);
  const host = new EventTarget();
  const heard: string[] = [];
  for (const name of ["graph-load", "node-select", "graph-error", "node-hover"]) {
    host.addEventListener(name, (event) => {
      if (event instanceof CustomEvent) heard.push(`${name} ${JSON.stringify(event.detail)}`);
    });
  }
  const handlers: Handlers = { hover: new Set(), select: new Set(), selection: new Set(), camera: new Set(), context: new Set(), frame: new Set() };
  const view = {
    on: <Name extends keyof ViewEvents>(name: Name, handler: (payload: ViewEvents[Name]) => void): (() => void) => {
      handlers[name].add(handler);
      return () => void handlers[name].delete(handler);
    },
  };
  watchHost({ host, store, view, previews: createPreviews({ resolver: () => null }), frames: scheduler });
  return { heard, store, hover: (node) => handlers.hover.forEach((handler) => handler(node)) };
}

/** The node-hover events of a pointer crossing a, b and c, with a frame after every move or none. */
function hoversAcross(framePerMove: boolean): string[] {
  const scheduler = frames();
  const subject = watched(DRAWN, scheduler);
  for (const node of [0, 1, 2]) {
    subject.hover(node);
    if (framePerMove) scheduler.flush();
  }
  scheduler.flush();
  return subject.heard;
}

test("node-hover is sent at most once a frame, for the node the pointer ended on", () => {
  assert.deepEqual(hoversAcross(false), ['node-hover {"id":"c"}']);
});

test("negative control: with a frame between every move the same crossing is three events", () => {
  assert.deepEqual(hoversAcross(true), ['node-hover {"id":"a"}', 'node-hover {"id":"b"}', 'node-hover {"id":"c"}']);
});

test("graph-load waits for the new graph's frame, then is sent once", () => {
  const subject = watched(DRAWN, frames());
  const graph = { name: "next", nodeCount: 3, edgeCount: 1, notes: ["n1", "n2"], buildMs: 1 };
  subject.store.set({ ...DRAWN, graph });
  assert.deepEqual(subject.heard, [], "the old frame is still on screen");
  subject.store.set({ ...DRAWN, graph, meta: { ...META } });
  subject.store.set({ ...DRAWN, graph, meta: { ...META } });
  assert.deepEqual(subject.heard, ['graph-load {"nodes":3,"edges":1,"notes":["n1","n2"]}']);
});

const hoversOf = (heard: readonly string[]): string[] => heard.filter((line) => line.startsWith("node-hover"));

/** A reset and a pointer move inside one frame, over a graph that is not the one on screen. */
test("a reset on a new graph shares the hover's frame, so the pair is one node-hover", () => {
  const scheduler = frames();
  const subject = watched(DRAWN, scheduler);
  const graph = (name: string) => ({ name, nodeCount: 3, edgeCount: 1, notes: ["n1", "n2"], buildMs: 1 });
  subject.hover(0);
  scheduler.flush();
  subject.store.set({ ...DRAWN, graph: graph("first"), meta: { ...META } });
  subject.hover(0);
  scheduler.flush();
  assert.deepEqual(hoversOf(subject.heard), ['node-hover {"id":"a"}'], "the reset and the hover are one frame");
  subject.store.set({ ...DRAWN, graph: graph("second"), meta: { ...META } });
  scheduler.flush();
  assert.deepEqual(hoversOf(subject.heard), ['node-hover {"id":"a"}', 'node-hover {"id":null}'], "a lone reset still lands");
});

/** A detail a host built with a cycle in it: the clone keeps the cycle, so the walk must as well. */
type CyclicSelect = HostEvents["node-select"] & {
  /** What the host hung off its detail; `node-select`'s own type has no such member. */
  readonly node: { readonly self?: unknown };
};

function cyclicSelect(): CyclicSelect {
  const node: { self?: unknown } = {};
  node.self = node;
  return { ids: ["a"], node };
}

/** The `node` of a listener's `node-select` detail, or null when it heard no such detail. */
function heardNode(event: Event): { readonly self?: unknown } | null {
  if (!(event instanceof CustomEvent)) return null;
  const detail: unknown = event.detail;
  if (typeof detail !== "object" || detail === null || !("node" in detail)) return null;
  const node: unknown = detail.node;
  return typeof node === "object" && node !== null && "self" in node ? node : null;
}

/** True when a walk gives up by throwing, which is what a walk with no visited set does on a cycle. */
function overflows(walk: () => unknown): boolean {
  try {
    walk();
    return false;
  } catch {
    return true;
  }
}

test("emit sends a detail that refers to itself instead of overflowing the stack", () => {
  const target = new EventTarget();
  const held: Event[] = [];
  target.addEventListener("node-select", (event) => void held.push(event));
  emit(target, "node-select", cyclicSelect());
  assert.equal(held.length, 1, "the event was dispatched, not thrown from");
  const [event] = held;
  assert.ok(event !== undefined, "the listener heard the event");
  const node = heardNode(event);
  assert.ok(node !== null, "and it carried the object the cycle is in");
  assert.equal(Object.isFrozen(node), true, "the object that holds the cycle is frozen");
  assert.equal(Object.isFrozen(node.self), true, "and so is what it points at");
});

test("negative control: the test's own walk has no visited set, and a cycle is what stops it", () => {
  const node: { self?: unknown } = {};
  node.self = node;
  Object.freeze(node);
  assert.equal(overflows(() => deepFrozen(node)), true);
  assert.equal(deepFrozen(Object.freeze({ ids: Object.freeze(["a"]) })), true);
});

const REFUSED: ShownError = { title: "RunRefusedError", code: "LayoutRefused", detail: "the layout refused", hint: "" };

test("graph-error is sent once per error, named by its code, else by its title", () => {
  const subject = watched(initialState(), frames());
  subject.store.set({ ...initialState(), error: REFUSED });
  subject.store.set({ ...subject.store.get(), selection: [] });
  subject.store.set({ ...initialState(), error: { ...REFUSED, code: null } });
  assert.deepEqual(subject.heard, [
    'graph-error {"error":"LayoutRefused","message":"the layout refused"}',
    'graph-error {"error":"RunRefusedError","message":"the layout refused"}',
  ]);
});
