/** The camera actions, run through the studio: what each one asks the view to do. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { desk } from "./desk.ts";
import { refusingClient } from "./ui/desk.ts";

test("fit, reset, zoom, pan and clear each ask the view for one named change", async () => {
  const made = desk(refusingClient());
  assert.equal((await made.studio.dispatch("view.fit")).ok, true);
  assert.equal((await made.studio.dispatch("view.reset")).ok, true);
  assert.equal((await made.studio.dispatch("view.zoom", { factor: 2 })).ok, true);
  assert.equal((await made.studio.dispatch("view.pan", { dx: 50, dy: -10 })).ok, true);
  assert.equal((await made.studio.dispatch("view.clear")).ok, true);
  assert.deepEqual(made.seen.calls, ["fit", "reset", "zoomBy 2", "panBy 50 -10", "select -1", "showAll"]);
});

test("a pan that is not a number is refused before the view is asked", async () => {
  const made = desk(refusingClient());
  const entry = await made.studio.dispatch("view.pan", { dx: "left", dy: 0 });
  assert.equal(entry.ok, false);
  assert.match(entry.error?.detail ?? "", /dx/);
  assert.deepEqual(made.seen.calls, []);
});

test("a pan beyond a screen is refused, so a typo cannot throw the view off", async () => {
  const made = desk(refusingClient());
  assert.equal((await made.studio.dispatch("view.pan", { dx: 4000, dy: 0 })).ok, false);
  assert.equal((await made.studio.dispatch("view.pan", { dx: 0, dy: -4000 })).ok, false);
  assert.deepEqual(made.seen.calls, []);
});

test("the zoom factor is bounded by the action's own numbers", async () => {
  const made = desk(refusingClient());
  assert.equal((await made.studio.dispatch("view.zoom", { factor: 0 })).ok, false);
  assert.equal((await made.studio.dispatch("view.zoom", { factor: 100 })).ok, false);
  assert.equal((await made.studio.dispatch("view.zoom", { factor: 40 })).ok, true);
  assert.deepEqual(made.seen.calls, ["zoomBy 40"]);
});

test("each camera action has a console name of its own", () => {
  const { studio } = desk(refusingClient());
  assert.equal(studio.registry.find("view.reset")?.alias, "reset");
  assert.equal(studio.registry.find("view.pan")?.alias, "pan");
  assert.equal(studio.registry.find("view.clear")?.alias, "deselect");
  assert.equal(studio.registry.find("view.zoom")?.alias, "zoom");
});
