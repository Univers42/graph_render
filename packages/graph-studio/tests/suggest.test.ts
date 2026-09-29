// What the console offers before it is asked: the word nearest a typo, and the rest of a
// line that Tab would complete. Both are read off the one registry, and neither runs
// anything: a suggestion is a word to retype, never a word that was run.
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Action, createRegistry } from "../src/actions/registry.ts";
import { complete, ghost } from "../src/console/complete.ts";
import { CommandRefusal, parseCommand } from "../src/console/parse.ts";

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
    id: "appearance.theme", alias: "theme", title: "Theme", section: "appearance",
    params: [{ name: "name", kind: "choice", title: "Theme", choices: () => ["dark", "light"], value: () => "dark" }],
    run: () => ({ message: "" }),
  },
  { id: "source.synthetic", alias: "synthetic", title: "Generate", section: "source", params: [], run: () => ({ message: "" }) },
  { id: "view.fit", alias: "fit", title: "Fit", section: null, params: [], run: () => ({ message: "" }) },
  { id: "view.reset", alias: "reset", title: "Reset", section: null, params: [], run: () => ({ message: "" }) },
];

const registry = createRegistry(ACTIONS);

function refused(text: string): string {
  try {
    parseCommand(text, registry);
  } catch (error) {
    if (error instanceof CommandRefusal) return error.message;
    throw error;
  }
  throw new Error(`\`${text}\` was not refused`);
}

const NEAR: readonly (readonly [string, string])[] = [
  ["layuot", "layout"],
  ["thme", "theme"],
  ["rest", "reset"],
  ["syntetic", "synthetic"],
];

for (const [typed, wanted] of NEAR) {
  test(`\`${typed}\` is refused with the nearest command, \`${wanted}\``, () => {
    assert.equal(registry.nearest(typed), wanted);
    assert.ok(refused(`${typed} x`).includes(`did you mean \`${wanted}\`?`), refused(`${typed} x`));
  });
}

test("a word further from every command than two edits is not guessed", () => {
  assert.equal(registry.nearest("synthe"), undefined);
  assert.equal(registry.nearest("zebra"), undefined);
  assert.equal(registry.nearest(""), undefined);
  assert.match(refused("zebra"), /`zebra` is not a command/);
  assert.doesNotMatch(refused("zebra"), /did you mean/);
});

test("both refusals name the listing that answers every one of them", () => {
  assert.match(refused("layuot x"), /`help` lists them all/);
  assert.match(refused("zebra x"), /`help` lists them all/);
});

test("a suggested word is a real command, and nothing is run from a suggestion", () => {
  const near = registry.nearest("thme") ?? "";
  assert.equal(registry.find(near)?.alias, "theme");
  assert.throws(() => parseCommand("thme light", registry), CommandRefusal);
});

test("the word itself is never a suggestion: it is a command", () => {
  assert.equal(registry.nearest("theme"), "theme");
  assert.equal(registry.nearest("LAYOUT"), "layout");
});

const GHOSTS: readonly (readonly [string, string])[] = [
  ["th", "eme "],
  ["fit", " "],
  ["", ""],
  ["synthetic 5", ""],
  ["nope x", ""],
  ["theme d", "ark "],
  ["theme dark", " "],
  ["layout atlas", ""],
  ["layout layout.force", ""],
  ["layout fo", ""],
  ["f", "it "],
];

for (const [typed, wanted] of GHOSTS) {
  test(`what the ghost would add to \`${typed}\` is \`${wanted}\``, () => {
    assert.equal(ghost(typed, registry, STATE), wanted);
  });
}

test("the ghost is exactly what Tab would add, or nothing", () => {
  for (const [typed] of [...GHOSTS, ["the", "me "], ["theme ", "dark, light"]]) {
    const shown = ghost(typed, registry, STATE);
    if (shown === "") continue;
    assert.equal(typed + shown, complete(typed, registry, STATE).text, typed);
  }
});

test("a ghost never shows a completion that would rewrite what was typed", () => {
  // Tab replaces the word; a ghost that deletes characters would be a lie about the line.
  const rewriting: readonly string[] = ["layout fo", "theme ark"];
  for (const typed of rewriting) {
    const done = complete(typed, registry, STATE).text;
    assert.notEqual(done, typed, typed);
    assert.equal(ghost(typed, registry, STATE), "", typed);
  }
});
