/** The view.local action, run through the studio: what it asks the view to do. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { desk } from "./desk.ts";
import { DRAWN, refusingClient } from "./ui/desk.ts";
import { initialState } from "../src/state/model.ts";

function drawn(): ReturnType<typeof desk> {
  const made = desk(refusingClient());
  made.studio.store.set(DRAWN);
  return made;
}

test("view.local asks the view for the local graph with the given parameters", async () => {
  const made = drawn();
  const result = await made.studio.dispatch("view.local", { id: "a", depth: 2, incoming: true, outgoing: false, neighbours: true });
  assert.equal(result.ok, true);
  assert.equal(result.message, "local graph depth 2");
  assert.equal(result.digest, JSON.stringify([0, 1, 2]));
  assert.deepEqual(made.seen.calls, ["local 0 {\"depth\":2,\"incoming\":true,\"outgoing\":false,\"neighbours\":true}"]);
});

test("view.local defaults missing parameters", async () => {
  const made = drawn();
  const result = await made.studio.dispatch("view.local", { id: "b" });
  assert.equal(result.ok, true);
  assert.equal(result.message, "local graph depth 1");
  assert.equal(result.digest, JSON.stringify([1]));
  assert.deepEqual(made.seen.calls, ["local 1 {\"depth\":1,\"incoming\":false,\"outgoing\":false,\"neighbours\":false}"]);
});

test("view.local validates depth bounds", async () => {
  const made = drawn();
  assert.equal((await made.studio.dispatch("view.local", { id: "a", depth: 0 })).ok, false);
  assert.equal((await made.studio.dispatch("view.local", { id: "a", depth: 6 })).ok, false);
  assert.equal((await made.studio.dispatch("view.local", { id: "a", depth: 1 })).ok, true);
  assert.equal((await made.studio.dispatch("view.local", { id: "a", depth: 5 })).ok, true);
  const call = "local 0 {\"depth\":1,\"incoming\":false,\"outgoing\":false,\"neighbours\":false}";
  assert.deepEqual(made.seen.calls, [call, call.replace('"depth":1', '"depth":5')]);
});

test("view.local refuses when no graph is drawn", async () => {
  const { studio } = desk(refusingClient(), initialState().settings);
  const result = await studio.dispatch("view.local", { id: "a" });
  assert.equal(result.ok, false);
  assert.match(result.error?.detail ?? "", /nothing is drawn/);
});

test("view.local is found by id and alias", () => {
  const { studio } = desk(refusingClient());
  assert.equal(studio.registry.find("view.local")?.alias, "local");
  assert.equal(studio.registry.find("local")?.id, "view.local");
});