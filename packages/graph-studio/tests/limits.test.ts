/** The size caps: a graph too large for the page is refused before anything is built for it. */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { StudioContext } from "../src/actions/context.ts";
import { ActionRefusal, createRegistry } from "../src/actions/registry.ts";
import { SOURCE_ACTIONS } from "../src/actions/source.ts";
import { IngestRefusal, normaliseIngest } from "../src/source/ingest.ts";
import { MAX_DOCUMENT_CHARS, MAX_LINKS, documentRefusal, linksRefusal } from "../src/source/limits.ts";
import { syntheticRecords } from "../src/source/synthetic.ts";
import { initialState } from "../src/state/model.ts";
import { type Settings, DEFAULT_SETTINGS } from "../src/state/settings.ts";
import { silentView } from "./silent-view.ts";

test("a document at the cap is opened, one character past it is refused with its size", () => {
  assert.equal(documentRefusal(MAX_DOCUMENT_CHARS), null);
  assert.match(documentRefusal(MAX_DOCUMENT_CHARS + 1) ?? "", /is 256\.0 Mi characters; the studio opens at most 256 Mi/);
});

test("links at the cap are generated, one past it are refused with the product", () => {
  assert.equal(linksRefusal(1_000_000, 2), null);
  assert.equal(linksRefusal(MAX_LINKS, 1), null);
  assert.match(linksRefusal(1_000_000, 3) ?? "", /1000000 nodes × 3 links per node is 3000000 links; .* at most 2000000/);
});

test("a generated graph past the cap is refused before a record is made", () => {
  assert.throws(() => syntheticRecords({ seed: 1, nodeCount: 1_000_000, degree: 3 }), IngestRefusal);
});

// `repeat` builds a rope, so the oversized text costs a few nodes, not 256 MiB, until it is flattened.
test("a document past the cap is refused before it is parsed", () => {
  const text = "x".repeat(MAX_DOCUMENT_CHARS + 1);
  assert.throws(() => normaliseIngest(text, "document"), (error: unknown) => error instanceof IngestRefusal && /at most 256 Mi/.test(error.message));
});

function context(applied: Settings[]): StudioContext {
  return {
    state: () => initialState(DEFAULT_SETTINGS),
    view: silentView([]),
    stop: () => false,
    save: () => undefined,
    clearLog: () => undefined,
    animation: { start: () => ({ message: "animating" }), cancel: () => ({ message: "cancelled" }) },
    actions: () => SOURCE_ACTIONS,
    recall: () => null,
    open: () => undefined,
    apply: (next) => Promise.resolve({ message: `${applied.push(next)}` }),
    look: () => ({ message: "restyled" }),
    bytes: () => null,
    neighbours: () => [],
    fitResults: () => ({ message: "fitted" }),
    reveal: () => undefined,
  };
}

test("the generate action refuses nodes × links past the cap, and opens the largest it allows", async () => {
  const registry = createRegistry(SOURCE_ACTIONS);
  const applied: Settings[] = [];
  const at = context(applied);
  const ask = (degree: number): ReturnType<typeof registry.resolve> => registry.resolve("synthetic", { nodes: 1_000_000, degree }, at.state());
  const refused = ask(3);
  assert.throws(() => refused.action.run(at, refused.args), (error: unknown) => error instanceof ActionRefusal && error.code === "bad-value");
  assert.equal(applied.length, 0);
  const allowed = ask(2);
  await allowed.action.run(at, allowed.args);
  assert.deepEqual(applied.map((settings) => settings.source.kind), ["synthetic"]);
});
