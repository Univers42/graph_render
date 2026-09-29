import assert from "node:assert/strict";
import { test } from "node:test";

import { type Action, ActionRefusal, createRegistry, matchChoice } from "../src/actions/registry.ts";

interface State {
  readonly layouts: readonly string[];
  readonly layout: string;
  readonly nodes: number;
  readonly dark: boolean;
  readonly loaded: boolean;
}

const STATE: State = {
  layouts: ["layout.grid", "layout.force.barnes_hut", "layout.forceatlas2", "layout.none"],
  layout: "layout.grid", nodes: 120, dark: true, loaded: false,
};

const ran: string[] = [];
const ACTIONS: readonly Action<State, null>[] = [
  {
    id: "layout.run", alias: "layout", title: "Run a layout", section: "layout",
    params: [{ name: "id", kind: "choice", title: "Layout", choices: (state) => state.layouts, value: (state) => state.layout }],
    run: (_context, args) => {
      ran.push(`layout ${String(args["id"])}`);
      return { message: "ran" };
    },
  },
  {
    id: "source.synthetic", alias: "synthetic", title: "Generate", section: "source",
    params: [
      { name: "nodes", kind: "int", title: "Nodes", min: 2, max: 500, value: (state) => state.nodes },
      { name: "ratio", kind: "number", title: "Ratio", min: 0, max: 1, value: () => 0.5 },
      { name: "name", kind: "text", title: "Name", value: () => "graph" },
      { name: "dark", kind: "flag", title: "Dark", value: (state) => state.dark },
    ],
    run: () => ({ message: "generated" }),
  },
  {
    id: "export.png", alias: "png", title: "Save a PNG", section: "export", params: [],
    available: (state) => (state.loaded ? null : "no graph is drawn yet"),
    run: () => ({ message: "saved" }),
  },
];

const registry = createRegistry(ACTIONS);

function refusal(code: string, pattern: RegExp): (error: unknown) => boolean {
  return (error) => error instanceof ActionRefusal && error.code === code && pattern.test(error.message);
}

test("an action is found by id and by alias", () => {
  assert.equal(registry.find("layout.run")?.id, "layout.run");
  assert.equal(registry.find("layout")?.id, "layout.run");
  assert.equal(registry.find("nope"), undefined);
});

test("two actions with one id, or one alias, are refused when the registry is made", () => {
  const [first] = ACTIONS;
  assert.ok(first !== undefined);
  assert.throws(() => createRegistry([first, first]), /layout.run/);
  assert.throws(() => createRegistry([first, { ...first, id: "layout.other" }]), /alias `layout`/);
});

test("an unknown action is refused, with the nearest names", () => {
  assert.throws(() => registry.resolve("layuot", {}, STATE), refusal("unknown-action", /`layuot`.*layout/));
});

test("a parameter the action does not take is refused", () => {
  assert.throws(() => registry.resolve("layout", { seed: 1 }, STATE), refusal("unknown-param", /`seed`.*id/));
});

test("a parameter left out takes the current value", () => {
  const { action, args } = registry.resolve("synthetic", { nodes: 40 }, STATE);
  assert.equal(action.id, "source.synthetic");
  assert.deepEqual(args, { nodes: 40, ratio: 0.5, name: "graph", dark: true });
});

test("text from the console is read as the parameter's kind", () => {
  const { args } = registry.resolve("synthetic", { nodes: "300", ratio: "0.25", name: 7, dark: "off" }, STATE);
  assert.deepEqual(args, { nodes: 300, ratio: 0.25, name: "7", dark: false });
});

const BAD_VALUES: readonly (readonly [string, Record<string, unknown>, RegExp])[] = [
  ["a fraction for a whole number", { nodes: "1.5" }, /`nodes`.*whole number/],
  ["text for a number", { ratio: "half" }, /`ratio`.*number/],
  ["a number below the range", { nodes: 1 }, /`nodes`.*2\.\.500/],
  ["a number above the range", { nodes: 501 }, /`nodes`.*2\.\.500/],
  ["a flag that is neither on nor off", { dark: "maybe" }, /`dark`.*on or off/],
  ["not-a-number", { ratio: Number.NaN }, /`ratio`.*number/],
  ["an object", { name: { a: 1 } }, /`name`/],
];

for (const [name, raw, pattern] of BAD_VALUES) {
  test(`${name} is refused, not clamped`, () => {
    assert.throws(() => registry.resolve("synthetic", raw, STATE), refusal("bad-value", pattern));
  });
}

test("a choice is matched exactly, then by the end of its id", () => {
  assert.equal(matchChoice("layout.grid", STATE.layouts), "layout.grid");
  assert.equal(matchChoice("forceatlas2", STATE.layouts), "layout.forceatlas2");
  assert.equal(matchChoice("barnes_hut", STATE.layouts), "layout.force.barnes_hut");
  assert.equal(matchChoice("force.barnes_hut", STATE.layouts), "layout.force.barnes_hut");
  assert.equal(matchChoice("GRID", STATE.layouts), "layout.grid");
});

test("a choice that fits two ids, or none, is not guessed", () => {
  assert.deepEqual(matchChoice("force", STATE.layouts), ["layout.force.barnes_hut", "layout.forceatlas2"]);
  assert.deepEqual(matchChoice("sugiyama", STATE.layouts), []);
  assert.throws(() => registry.resolve("layout", { id: "force" }, STATE), refusal("bad-value", /barnes_hut.*forceatlas2/));
  assert.throws(() => registry.resolve("layout", { id: "sugiyama" }, STATE), refusal("bad-value", /layout.grid/));
});

test("an unavailable action is refused with its reason, and an available one is not", () => {
  assert.throws(() => registry.resolve("png", {}, STATE), refusal("unavailable", /no graph is drawn yet/));
  assert.equal(registry.resolve("png", {}, { ...STATE, loaded: true }).action.id, "export.png");
});

test("resolving runs nothing", () => {
  registry.resolve("layout", { id: "grid" }, STATE);
  assert.deepEqual(ran, []);
});
