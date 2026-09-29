// Gate row `actions-parity`: the dock, the console and a host reach the same actions with
// the same values, because all three are read off the one list.
import assert from "node:assert/strict";
import { test } from "node:test";

import { DOCK_SECTIONS, studioActions } from "../src/actions/all.ts";
import type { StudioContext } from "../src/actions/context.ts";
import { createRegistry } from "../src/actions/registry.ts";
import { formatCommand, parseCommand } from "../src/console/parse.ts";
import { metaOf } from "../src/source/meta.ts";
import { type RunSummary, type StudioState, initialState } from "../src/state/model.ts";
import { controlOf } from "../src/ui/controlOf.ts";

const CATALOG = {
  layouts: ["layout.forceatlas2", "layout.grid"],
  posts: ["post.style.bezier"],
  analyses: ["analysis.depth.bfs"],
};
const STATE: StudioState = { ...initialState(), catalog: CATALOG };
const NODES = ["a", "b"].map((id) => ({
  id, kind: "note" as const, database_id: null, source: "file", label: id.toUpperCase(), group: `group ${id}`,
  weight: 1, version: 1, has_note: true, icon: null,
}));
const RUN: RunSummary = {
  layoutId: "layout.grid", postId: null, postError: null, digest: "00".repeat(32), byteLength: 1,
  nodeKind: "Point", edgeKind: "Line", layoutMs: 1, postMs: 0, notes: [],
};
const DRAWN: StudioState = {
  ...STATE,
  graph: { name: "two", nodeCount: 2, edgeCount: 1, notes: [], buildMs: 1 },
  meta: metaOf(NODES, ["a", "b"], { source: Uint32Array.of(0), target: Uint32Array.of(1) }),
  run: RUN,
};
const registry = createRegistry<StudioState, StudioContext>(studioActions());

test("ids and console words are unique, and every word is one word", () => {
  const words = registry.actions.map((action) => action.alias);
  assert.equal(new Set(words).size, words.length);
  assert.deepEqual(words.filter((word) => !/^[a-z]+$/.test(word)), []);
  assert.deepEqual(registry.actions.filter((action) => !/^[a-z]+(\.[a-z]+)+$|^help$/.test(action.id)), []);
});

test("the line the log prints for an action parses back to the same action and values", () => {
  for (const action of registry.actions) {
    const args = Object.fromEntries(action.params.map((spec) => [spec.name, spec.value(STATE)]));
    const command = parseCommand(formatCommand(action, args), registry);
    assert.equal(command.id, action.id);
    const typed = Object.fromEntries(action.params.map((spec) => [spec.name, String(args[spec.name])]));
    assert.deepEqual(command.raw, typed, action.id);
  }
});

test("every section of the dock shows an action, and every shown action is in a section", () => {
  const shown = registry.actions.filter((action) => action.section !== null);
  assert.deepEqual([...new Set(shown.map((action) => action.section))], [...DOCK_SECTIONS]);
});

test("every parameter has a control, and a choice control has choices to show", () => {
  for (const action of registry.actions) {
    assert.equal(action.available?.(DRAWN) ?? null, null, action.id);
    for (const spec of action.params) {
      const control = controlOf(spec);
      if (spec.kind === "choice") assert.ok((spec.choices?.(DRAWN) ?? []).length > 0, `${action.id}.${spec.name}`);
      if (control === "slider") assert.ok(spec.min !== undefined && spec.max !== undefined, `${action.id}.${spec.name}`);
      assert.equal(typeof spec.title, "string");
    }
  }
});

test("a parameter's default is a value the action accepts", () => {
  for (const state of [STATE, DRAWN]) {
    for (const action of registry.actions) {
      const raw = Object.fromEntries(action.params.map((spec) => [spec.name, spec.value(state)]));
      const free = action.available?.(state) ?? null;
      if (free === null) assert.doesNotThrow(() => registry.resolve(action.id, raw, state), action.id);
      else assert.throws(() => registry.resolve(action.id, raw, state), /cannot run/, action.id);
    }
  }
});

test("what is unavailable says why", () => {
  assert.throws(() => registry.resolve("export.recipe", {}, STATE), /`recipe` cannot run: nothing is drawn/);
});
