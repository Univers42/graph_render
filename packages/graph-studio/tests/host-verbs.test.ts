/**
 * Row `host-api-unit`, the verbs (`docs/contract/host-api.md`, verdicts 2, 7 and 10): exact ids
 * only, a selection that does not change announces nothing, and a refused `loadGraph` names
 * itself as its `graph-error` does. Each check carries its negative control beside it.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type HostVerbs, hostVerbs } from "../src/host/api.ts";
import { createPreviews } from "../src/host/previews.ts";
import { watchHost } from "../src/host/watch.ts";
import type { MotorClient } from "../src/motor/client.ts";
import type { Studio } from "../src/studio/studio.ts";
import { desk, scriptedClient } from "./desk.ts";
import { DRAWN } from "./drawn.ts";

const drained = (): Promise<void> => new Promise((done) => setImmediate(done));

interface Bench {
  readonly studio: Studio;
  readonly verbs: HostVerbs;
  readonly calls: string[];
  /** Every event the element would have sent, as `name detail-json`. */
  readonly heard: string[];
}

const EVENTS = ["graph-load", "node-select", "graph-error", "node-hover"] as const;

/** A studio over `client`, its host events recorded; `state` set by hand when given. */
function bench(client: MotorClient = scriptedClient(), drawn = true): Bench {
  const made = desk(client);
  if (drawn) made.studio.store.set(DRAWN);
  const host = new EventTarget();
  const heard: string[] = [];
  for (const name of EVENTS) {
    host.addEventListener(name, (event) => {
      if (event instanceof CustomEvent) heard.push(`${name} ${JSON.stringify(event.detail)}`);
    });
  }
  const view = { on: () => () => undefined };
  watchHost({ host, store: made.studio.store, view, previews: createPreviews({ resolver: () => null }) });
  return { studio: made.studio, verbs: hostVerbs(made.studio, Promise.resolve()), calls: made.seen.calls, heard };
}

/** What `move` made the view do, after every command it dispatched has run. */
async function viewCalls(move: (subject: Bench) => unknown): Promise<string[]> {
  const subject = bench();
  move(subject);
  await drained();
  return subject.calls.filter((call) => call.startsWith("focus") || call.startsWith("select"));
}

test("focusNode takes an exact id, and a label, an unknown id or a non-string moves nothing", async () => {
  assert.deepEqual(await viewCalls(({ verbs }) => verbs.focusNode("b")), ["focus 1"]);
  for (const id of ["Alpha", "zz", "", 1, null]) {
    assert.deepEqual(await viewCalls(({ verbs }) => assert.equal(verbs.focusNode(id), false)), [], String(id));
  }
});

test("negative control: the console's `focus`, which matches labels, moves on the same string", async () => {
  assert.deepEqual(await viewCalls(({ studio }) => studio.dispatch("view.focus", { node: "Alpha" })), ["focus 0"]);
});

test("focusNode with nothing drawn answers false", async () => {
  const subject = bench(scriptedClient(), false);
  assert.equal(subject.verbs.focusNode("a"), false);
  await drained();
  assert.deepEqual(subject.calls, []);
});

test("selectNodes takes exact ids, the last one primary, and refuses the whole call on one unknown", async () => {
  assert.deepEqual(await viewCalls(({ verbs }) => verbs.selectNodes(["c", "a", "c"])), ["selectMany 2,0"]);
  for (const ids of [["a", "Beta"], ["a", "zz"], "a", [1], null]) {
    assert.deepEqual(await viewCalls(({ verbs }) => assert.equal(verbs.selectNodes(ids), false)), [], JSON.stringify(ids));
  }
});

/** The `node-select` events heard while `second` ran after a first selection of a and c. */
async function selectsAfter(second: readonly string[]): Promise<string[]> {
  const subject = bench();
  assert.equal(subject.verbs.selectNodes(["a", "c"]), true);
  await drained();
  assert.equal(subject.verbs.selectNodes(second), true);
  await drained();
  return subject.heard.filter((line) => line.startsWith("node-select"));
}

test("node-select fires on a change only, never for a selectNodes with the current set", async () => {
  assert.deepEqual(await selectsAfter(["c", "a"]), ['node-select {"ids":["a","c"]}']);
});

test("negative control: a different set is announced, so the count above can move", async () => {
  assert.deepEqual(await selectsAfter(["b"]), ['node-select {"ids":["a","c"]}', 'node-select {"ids":["b"]}']);
});

test("selectedIds is a frozen list of the host's ids, in selection order", async () => {
  const subject = bench();
  subject.verbs.selectNodes(["c", "a"]);
  await drained();
  const ids = subject.verbs.selectedIds();
  assert.deepEqual(ids, ["c", "a"]);
  assert.equal(Object.isFrozen(ids), true);
  assert.equal(subject.verbs.selectNodes([]), true);
  await drained();
  assert.deepEqual(subject.verbs.selectedIds(), []);
});

test("loadGraph refuses what is not an object, or what serialises to nothing, before the studio sees it", async () => {
  const cyclic: { self?: unknown } = {};
  cyclic.self = cyclic;
  for (const doc of [42, "{}", null, cyclic, { toJSON: () => undefined }]) {
    await assert.rejects(bench().verbs.loadGraph(doc), TypeError);
  }
});
