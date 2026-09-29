import assert from "node:assert/strict";
import { test } from "node:test";

import { type Action, createRegistry } from "../src/actions/registry.ts";
import { complete } from "../src/console/complete.ts";
import { CommandRefusal, formatCommand, parseCommand, tokenise } from "../src/console/parse.ts";

interface State {
  readonly layouts: readonly string[];
}

const STATE: State = { layouts: ["layout.grid", "layout.force.barnes_hut", "layout.forceatlas2"] };

const ACTIONS: readonly Action<State, null>[] = [
  {
    id: "layout.run", alias: "layout", title: "Run a layout", section: "layout",
    params: [{ name: "id", kind: "choice", title: "Layout", choices: (state) => state.layouts, value: () => "layout.grid" }],
    run: () => ({ message: "" }),
  },
  {
    id: "source.synthetic", alias: "synthetic", title: "Generate", section: "source",
    params: [
      { name: "nodes", kind: "int", title: "Nodes", value: () => 120 },
      { name: "degree", kind: "int", title: "Degree", value: () => 2 },
    ],
    run: () => ({ message: "" }),
  },
  {
    id: "filter.text", alias: "filter", title: "Filter", section: "filter",
    params: [{ name: "text", kind: "text", title: "Text", value: () => "" }],
    run: () => ({ message: "" }),
  },
  { id: "view.fit", alias: "fit", title: "Fit", section: null, params: [], run: () => ({ message: "" }) },
];

const registry = createRegistry(ACTIONS);

test("words split on spaces, and quotes keep a space", () => {
  assert.deepEqual(tokenise("layout  forceatlas2"), ["layout", "forceatlas2"]);
  assert.deepEqual(tokenise('filter "graph notes"'), ["filter", "graph notes"]);
  assert.deepEqual(tokenise("filter text='a b' "), ["filter", "text=a b"]);
  assert.deepEqual(tokenise('filter ""'), ["filter", ""]);
  assert.deepEqual(tokenise("   "), []);
});

test("a quote that never closes is refused", () => {
  assert.throws(() => tokenise('filter "graph'), CommandRefusal);
});

test("a command is an alias, then values by position or by name", () => {
  assert.deepEqual(parseCommand("layout forceatlas2", registry), { id: "layout.run", raw: { id: "forceatlas2" } });
  assert.deepEqual(parseCommand("synthetic 500 3", registry), { id: "source.synthetic", raw: { nodes: "500", degree: "3" } });
  assert.deepEqual(parseCommand("synthetic degree=3", registry), { id: "source.synthetic", raw: { degree: "3" } });
  assert.deepEqual(parseCommand("synthetic 500 degree=3", registry), { id: "source.synthetic", raw: { nodes: "500", degree: "3" } });
  assert.deepEqual(parseCommand("layout.run id=grid", registry), { id: "layout.run", raw: { id: "grid" } });
  assert.deepEqual(parseCommand("fit", registry), { id: "view.fit", raw: {} });
});

test("a value may hold an equals sign when it is quoted or named", () => {
  assert.deepEqual(parseCommand("filter text=a=b", registry), { id: "filter.text", raw: { text: "a=b" } });
});

const REFUSED: readonly (readonly [string, string, RegExp])[] = [
  ["an empty line", "  ", /nothing to run/],
  ["an unknown command", "layuot grid", /`layuot`/],
  ["one value too many", "layout grid force", /`layout` takes 1 value/],
  ["a value given twice", "synthetic 500 nodes=3", /`nodes` is given twice/],
];

for (const [name, text, pattern] of REFUSED) {
  test(`${name} is refused`, () => {
    assert.throws(() => parseCommand(text, registry), (error: unknown) => error instanceof CommandRefusal && pattern.test(error.message));
  });
}

test("a command written from an action and its values reads back as the same thing", () => {
  const [layout, synthetic, filter] = ACTIONS;
  assert.ok(layout !== undefined && synthetic !== undefined && filter !== undefined);
  assert.equal(formatCommand(layout, { id: "layout.forceatlas2" }), "layout layout.forceatlas2");
  assert.equal(formatCommand(synthetic, { nodes: 500, degree: 3 }), "synthetic 500 3");
  assert.equal(formatCommand(filter, { text: "graph notes" }), 'filter "graph notes"');
  assert.equal(formatCommand(filter, { text: "" }), 'filter ""');
  assert.deepEqual(parseCommand(formatCommand(filter, { text: 'say "hi"' }), registry), { id: "filter.text", raw: { text: 'say "hi"' } });
});

test("completing the first word offers the commands that start with it", () => {
  assert.deepEqual(complete("", registry, STATE).candidates, ["filter", "fit", "layout", "synthetic"]);
  assert.deepEqual(complete("f", registry, STATE).candidates, ["filter", "fit"]);
  assert.deepEqual(complete("la", registry, STATE), { candidates: ["layout"], text: "layout " });
});

test("completing a value offers the choices that contain what was typed", () => {
  assert.deepEqual(complete("layout ", registry, STATE).candidates, STATE.layouts);
  assert.deepEqual(complete("layout force", registry, STATE).candidates, ["layout.force.barnes_hut", "layout.forceatlas2"]);
  assert.deepEqual(complete("layout atlas", registry, STATE), { candidates: ["layout.forceatlas2"], text: "layout layout.forceatlas2 " });
  assert.deepEqual(complete("layout id=gr", registry, STATE), { candidates: ["layout.grid"], text: "layout id=layout.grid " });
});

test("completing offers the shared start of several candidates", () => {
  assert.equal(complete("fi", registry, STATE).text, "fi");
  assert.equal(complete("layout layout.force", registry, STATE).text, "layout layout.force");
  assert.equal(complete("layout fo", registry, STATE).text, "layout layout.force");
});

test("completing a value with no choices, or an unknown command, offers nothing", () => {
  assert.deepEqual(complete("synthetic 5", registry, STATE), { candidates: [], text: "synthetic 5" });
  assert.deepEqual(complete("nope x", registry, STATE), { candidates: [], text: "nope x" });
});
