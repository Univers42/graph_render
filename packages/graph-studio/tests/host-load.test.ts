/**
 * Row `host-api-unit`, `loadGraph` against a scripted motor (`docs/contract/host-api.md`, verdict
 * 7): what it resolves with, the one `graph-load` it causes, the name a refusal carries, and the
 * `CancelledError` of a call that a second one overtook.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { hostVerbs } from "../src/host/api.ts";
import { createPreviews } from "../src/host/previews.ts";
import { watchHost } from "../src/host/watch.ts";
import type { MotorClient } from "../src/motor/client.ts";
import type { StudioState } from "../src/state/model.ts";
import { DEFAULT_SETTINGS, type Settings, withSettings } from "../src/state/settings.ts";
import type { Store } from "../src/state/store.ts";
import { SCRIPTED_META, desk, scriptedClient } from "./desk.ts";

const DOC = { nodes: [{ id: "a" }, { id: "b" }, { id: "c" }], edges: [] };

/**
 * The scripted motor, with a fresh meta object per run as the worker's decoder gives: the
 * scripted one hands back the same object every time, and a new graph is told apart by it.
 */
function freshMeta(client: MotorClient): MotorClient {
  return { ...client, layout: async (layout, post) => ({ ...(await client.layout(layout, post)), meta: { ...SCRIPTED_META } }) };
}

async function started(client: MotorClient, settings?: Settings): Promise<{ readonly heard: string[]; readonly load: (doc: unknown) => Promise<unknown>; readonly store: Store<StudioState> }> {
  const made = desk(freshMeta(client), settings);
  const entry = await made.studio.start();
  assert.equal(entry.ok, true, entry.message);
  const host = new EventTarget();
  const heard: string[] = [];
  for (const name of ["graph-load", "graph-error"]) {
    host.addEventListener(name, (event) => {
      if (event instanceof CustomEvent) heard.push(`${name} ${JSON.stringify(event.detail)}`);
    });
  }
  watchHost({ host, store: made.studio.store, view: { on: () => () => undefined }, previews: createPreviews({ resolver: () => null }) });
  const verbs = hostVerbs(host, made.studio, Promise.resolve());
  return { heard, load: (doc) => verbs.loadGraph(doc), store: made.studio.store };
}

test("loadGraph resolves with the counts and the notes, frozen, and graph-load says the same once", async () => {
  const subject = await started(scriptedClient());
  const result = await subject.load(DOC);
  assert.deepEqual(result, { nodes: 3, edges: 2, notes: [] });
  assert.equal(Object.isFrozen(result), true);
  assert.deepEqual(subject.heard, ['graph-load {"nodes":3,"edges":2,"notes":[]}']);
});

/** A motor whose load is refused the way the wasm refuses a document over its cap. */
function refusing(): MotorClient {
  const scripted = scriptedClient();
  let loads = 0;
  const refusal = Object.assign(new Error("the document is over the motor's cap"), { name: "BuildRefusedError", code: "IngestTooLarge" });
  // The first load is the studio's own start; the host's is the second.
  return { ...scripted, load: (source) => (loads++ === 0 ? scripted.load(source) : Promise.reject(refusal)) };
}

test("a refused loadGraph rejects with the name its graph-error carries, and no graph-load", async () => {
  const subject = await started(refusing());
  const error: unknown = await subject.load(DOC).then(() => null, (refused: unknown) => refused);
  assert.ok(error instanceof Error);
  assert.equal(error.name, "IngestTooLarge");
  assert.deepEqual(subject.heard.map((line) => line.split(" ")[0]), ["graph-error"]);
  assert.match(subject.heard[0] ?? "", /"error":"IngestTooLarge"/);
});

/**
 * A motor that takes the host's document, describes it under another name, and then refuses to
 * lay it out — and that measures the graph it opened with, so the store holds an analysis before
 * the load. This is the failure that reaches the pipeline's clearing: by then `load` has already
 * put the new graph, a null analysis and the new source on the store, and nothing was drawn.
 */
function unlaid(): MotorClient {
  const scripted = scriptedClient();
  let loads = 0;
  let layouts = 0;
  const refused = new Error("the motor could not lay the document out");
  return {
    ...scripted,
    load: (source) => (loads++ === 0 ? scripted.load(source) : Promise.resolve({ name: "handed over", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 })),
    layout: (layout, post, params) => (layouts++ === 0 ? scripted.layout(layout, post, params) : Promise.reject(refused)),
    analysis: (id) => Promise.resolve({ id, kind: "f64", values: Float64Array.of(0.1, 0.2, 0.3), converged: true, modularity: 0.3, max: 0.3, ms: 1 }),
  };
}

test("a fresh load that cannot be laid out rolls the store back to the graph the drawing is of", async () => {
  const subject = await started(unlaid(), withSettings(DEFAULT_SETTINGS, { analysis: "analysis.modularity" }));
  const before = subject.store.get();
  await assert.rejects(subject.load(DOC));
  const after = subject.store.get();
  // The frame is emptied: the drawing on screen was of another graph, and the run never came.
  assert.equal(after.meta, null);
  assert.equal(after.run, null);
  assert.deepEqual(after.graph, before.graph);
  assert.deepEqual(after.analysis, before.analysis);
  assert.deepEqual(after.settings.source, before.settings.source);
});

/**
 * A motor that holds the host's first load until the second one arrives, then rejects it the
 * way the client rejects a cancelled request.
 */
function overtaking(): MotorClient {
  const scripted = scriptedClient();
  const held: { reject: ((error: Error) => void) | null } = { reject: null };
  let loads = 0;
  return {
    ...scripted,
    load: (source) => {
      loads += 1;
      if (loads !== 2) return scripted.load(source);
      return new Promise((_, reject) => (held.reject = reject));
    },
    busy: () => held.reject !== null,
    cancel: () => {
      held.reject?.(Object.assign(new Error("cancelled"), { name: "CancelledError" }));
      held.reject = null;
      return true;
    },
  };
}

test("an overtaken loadGraph rejects with CancelledError, and only the second is announced", async () => {
  const subject = await started(overtaking());
  const first = subject.load(DOC).then(() => "resolved", (error: unknown) => (error instanceof Error ? error.name : "?"));
  await new Promise((done) => setImmediate(done));
  const second = await subject.load({ ...DOC, edges: [{ source: "a", target: "b" }] });
  assert.equal(await first, "CancelledError");
  assert.deepEqual(second, { nodes: 3, edges: 2, notes: [] });
  assert.deepEqual(subject.heard, ['graph-load {"nodes":3,"edges":2,"notes":[]}']);
});
