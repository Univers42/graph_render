// Gate row `recipe-replay`, the pure half: what a recipe holds, what is refused on the
// way back in, and what counts as the same drawing.
import assert from "node:assert/strict";
import { test } from "node:test";

import { type LogEntry, type RunSummary, initialState, withEntry } from "../src/state/model.ts";
import { RecipeMismatch, checkRecipe, readRecipe, recipeOf } from "../src/state/recipe.ts";
import { DEFAULT_SETTINGS } from "../src/state/settings.ts";

const DIGEST = "a".repeat(64);
const RUN: RunSummary = {
  layoutId: "layout.forceatlas2", postId: null, postError: null, digest: DIGEST, byteLength: 20188,
  nodeKind: "Point", edgeKind: "Line", dim: 0, layoutMs: 45, postMs: 0, notes: [],
};
const GRAPH = { name: "vault seed 1", nodeCount: 400, edgeCount: 798, notes: [], buildMs: 5 };

function entry(seq: number, command: string, ok: boolean): LogEntry {
  return { seq, command, ok, ms: 1, message: "", digest: null, notes: [], error: null };
}

function drawn(): ReturnType<typeof initialState> {
  const state = { ...initialState(), graph: GRAPH, run: RUN };
  return [entry(1, "synthetic 400 2 1 vault", true), entry(2, "layuot", false), entry(3, "theme light", true)]
    .reduce(withEntry, state);
}

test("a recipe holds the settings, what was drawn and the commands that worked, in order", () => {
  const recipe = recipeOf(drawn());
  assert.deepEqual(recipe, {
    version: 1,
    settings: DEFAULT_SETTINGS,
    expect: { digest: DIGEST, nodeCount: 400, edgeCount: 798 },
    log: ["synthetic 400 2 1 vault", "theme light"],
  });
});

test("nothing drawn, no recipe", () => {
  assert.throws(() => recipeOf(initialState()), /nothing is drawn/);
});

test("a recipe read back is the recipe written, frozen", () => {
  const recipe = recipeOf(drawn());
  const back = readRecipe(JSON.stringify(recipe));
  assert.deepEqual(back, recipe);
  assert.ok(Object.isFrozen(back.settings) && Object.isFrozen(back.settings.appearance));
});

test("what is not a recipe is refused by the member that was wrong", () => {
  const recipe = recipeOf(drawn());
  assert.throws(() => readRecipe("{"), /^SettingsRefusal: recipe: not JSON/);
  assert.throws(() => readRecipe(JSON.stringify({ ...recipe, version: 2 })), /recipe.version: not 1/);
  assert.throws(() => readRecipe(JSON.stringify({ ...recipe, camera: {} })), /recipe.camera: not a member/);
  const settings = { ...recipe.settings, layout: 7 };
  assert.throws(() => readRecipe(JSON.stringify({ ...recipe, settings })), /recipe.settings.layout: not a string/);
  const expect = { ...recipe.expect, nodeCount: -1 };
  assert.throws(() => readRecipe(JSON.stringify({ ...recipe, expect })), /recipe.expect.nodeCount/);
});

test("the same digest and counts is the same drawing; anything else is named", () => {
  const { expect } = recipeOf(drawn());
  assert.deepEqual(checkRecipe(expect, expect), []);
  assert.throws(
    () => checkRecipe(expect, { ...expect, digest: "b".repeat(64) }),
    (error) => error instanceof RecipeMismatch && error.message.includes("aaaaaaaa") && error.message.includes("bbbbbbbb"),
  );
  assert.throws(() => checkRecipe(expect, { ...expect, nodeCount: 399 }), /400 nodes.*399/);
});

test("without a digest on either side the check says it did not happen", () => {
  const { expect } = recipeOf(drawn());
  assert.deepEqual(checkRecipe({ ...expect, digest: null }, expect), ["digest NOT checked: the recipe holds none"]);
  assert.deepEqual(checkRecipe(expect, { ...expect, digest: null }), ["digest NOT checked: this platform offers no digest"]);
});

test("the log keeps the newest entries and drops the oldest", () => {
  let state = initialState();
  for (let seq = 1; seq <= 510; seq += 1) state = withEntry(state, entry(seq, `fit`, true));
  assert.deepEqual([state.log.length, state.log[0]?.seq, state.log.at(-1)?.seq], [500, 11, 510]);
});
