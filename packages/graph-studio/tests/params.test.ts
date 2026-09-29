// `set` and `get` are not a second parameter list: they address the parameters the
// registry already holds, and change them by running the action that holds them.
import assert from "node:assert/strict";
import { test } from "node:test";

import { studioActions } from "../src/actions/all.ts";
import { createRegistry } from "../src/actions/registry.ts";
import { parseCommand } from "../src/console/parse.ts";
import type { MotorClient } from "../src/motor/client.ts";
import { metaOf } from "../src/source/meta.ts";
import { type RunSummary, type StudioState, initialState } from "../src/state/model.ts";
import { type Desk, desk } from "./desk.ts";

const NODES = ["a", "b", "c"].map((id) => ({
  id, kind: "note" as const, database_id: null, source: "file", label: id, group: `group ${id}`,
  weight: 1, version: 1, has_note: false, icon: null,
}));
const RUN: RunSummary = {
  layoutId: "layout.forceatlas2", postId: null, postError: null, digest: "00".repeat(32), byteLength: 1,
  nodeKind: "Point", edgeKind: "Line", layoutMs: 1, postMs: 0, notes: [],
};
const DRAWN: StudioState = {
  ...initialState(),
  catalog: { layouts: ["layout.forceatlas2", "layout.grid"], posts: ["post.style.bezier"], analyses: ["analysis.depth.bfs"] },
  graph: { name: "three", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 },
  meta: metaOf(NODES, ["a", "b", "c"], { source: Uint32Array.of(0, 1), target: Uint32Array.of(1, 2) }),
  run: RUN,
};
const ACTIONS = studioActions();
const registry = createRegistry<StudioState, never>(ACTIONS);

/** Nothing in this file asks the motor: a call would fail the test rather than pass it. */
function quietClient(): MotorClient {
  const never = (what: string): never => {
    throw new Error(`the test client was asked to ${what}`);
  };
  return {
    catalog: () => never("open"), load: () => never("load"), layout: () => never("lay out"),
    analysis: () => never("analyse"), cancel: () => false, busy: () => false, close: () => undefined,
  };
}

function made(state: StudioState = DRAWN): Desk {
  const made = desk(quietClient());
  made.studio.store.set(state);
  return made;
}

function addressable(): readonly string[] {
  const set = registry.find("set");
  assert.ok(set !== undefined, "the registry holds `set`");
  return set.params[0]?.choices?.(DRAWN) ?? [];
}

const LIVE: readonly (readonly [string, string, string])[] = [
  ["theme.name", "light", "light"],
  ["colour.by", "kind", "kind"],
  ["size.by", "degree", "degree"],
  ["scale.factor", "2", "2"],
  ["labels.mode", "more", "more"],
  ["filter.text", "vault", "vault"],
  ["mindegree.min", "2", "2"],
  ["group.name", '"group a"', "group a"],
];

for (const [param, value, read] of LIVE) {
  test(`\`set ${param} ${value}\` is applied live, and \`get\` reads it back`, async () => {
    const { studio } = made();
    const set = await studio.run(`set ${param} ${value}`);
    assert.equal(set.ok, true, set.message);
    const got = await studio.run(`get ${param}`);
    assert.deepEqual([got.ok, got.message], [true, `${param} = ${read}`]);
  });
}

test("every parameter the registry holds is settable, gettable and named the same way", () => {
  const held = ACTIONS.filter((action) => action.alias !== "set" && action.alias !== "get")
    .flatMap((action) => action.params.map((spec) => `${action.alias}.${spec.name}`));
  const listed = addressable();
  assert.deepEqual([...listed].sort(), [...held].sort());
  assert.equal(new Set(listed).size, listed.length, "no parameter is listed twice");
});

test("`get` with no parameter reads the first one the registry holds", async () => {
  const { studio } = made();
  const got = await studio.run("get");
  assert.deepEqual([got.ok, got.message], [true, `${addressable()[0]} = 400`]);
});

test("a value the parameter does not take is refused by the registry, not by `set`", async () => {
  const { studio } = made();
  const bad = await studio.run("set scale.factor 99");
  assert.deepEqual([bad.ok, bad.error?.title], [false, "ActionRefusal"]);
  assert.match(bad.message, /`factor` must be in 0.25\.\.4, not 99/);
  const notOne = await studio.run("set theme.name neon");
  assert.match(notOne.message, /`name` is not one of: dark, light/);
  assert.equal(studio.store.get().settings.appearance.nodeScale, 1);
});

test("a parameter no action holds is refused, and every parameter is named", async () => {
  const { studio } = made();
  const none = await studio.run("set theme.colour light");
  assert.deepEqual([none.ok, none.error?.title], [false, "ActionRefusal"]);
  assert.match(none.message, /`param` is not one of: .*theme\.name, colour\.by/);
  assert.match((await studio.run("get gravity")).message, /is not one of/);
  assert.equal(studio.store.get().settings.appearance.theme, "dark", "nothing changed");
});

test("a parameter of an action that cannot run now is refused with its reason", async () => {
  const { studio } = made(initialState());
  const entry = await studio.run("set filter.text vault");
  assert.match(entry.message, /`filter` cannot run: nothing is drawn/);
  const motor = await studio.run("set layout.id layout.grid");
  assert.match(motor.message, /`layout` cannot run: no graph is loaded/);
});

test("`get` reads what the action would take, not what the camera is doing now", async () => {
  const { studio, seen } = made();
  const set = await studio.run("set zoom.factor 2");
  assert.equal(set.ok, true, set.message);
  assert.deepEqual(seen.calls, ["zoomBy 2"], "the camera moved");
  const got = await studio.run("get zoom.factor");
  assert.equal(got.message, "zoom.factor = 1.25", "the factor a bare `zoom` uses, not a zoom level");
});

test("`get` names the choices a parameter takes", async () => {
  const { studio } = made();
  assert.deepEqual((await studio.run("get theme.name")).notes, ["theme.name is one of: dark, light"]);
});

test("`set` and the dock's own control change the drawing the same way", async () => {
  const bySet = made();
  const set = await bySet.studio.run("set theme.name light");
  const byControl = made();
  // What ui/ActionForm.tsx calls when the theme control is used.
  const control = await byControl.studio.dispatch("theme", { name: "light" });
  const typed = await byControl.studio.run("theme light");
  assert.equal(set.command, "set theme.name light", "the log prints the line that was typed");
  assert.equal(control.command, "theme light", "and the line the panel would have printed");
  assert.equal(control.command, typed.command, "a control and a typed line log the same");
  assert.deepEqual(
    parseCommand(control.command, byControl.studio.registry),
    parseCommand("theme light", byControl.studio.registry),
  );
  assert.deepEqual(bySet.studio.store.get().settings, byControl.studio.store.get().settings);
});
