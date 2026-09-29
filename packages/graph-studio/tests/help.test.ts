// `help` is the listing, and it is read off the one registry: a command that exists is
// listed and one that does not is not, so the listing and the dock cannot drift apart.
import assert from "node:assert/strict";
import { test } from "node:test";

import { DOCK_SECTIONS, studioActions } from "../src/actions/all.ts";
import { createRegistry } from "../src/actions/registry.ts";
import { listing } from "../src/console/help.ts";
import { type StudioState, initialState } from "../src/state/model.ts";

const CATALOG = { layouts: ["layout.forceatlas2", "layout.grid"], posts: ["post.style.bezier"], analyses: ["analysis.depth.bfs"] };
const STATE: StudioState = { ...initialState(), catalog: CATALOG };
const ACTIONS = studioActions();
const registry = createRegistry<StudioState, never>(ACTIONS);

function lines(word: string): readonly string[] {
  return listing(ACTIONS, STATE, word).notes ?? [];
}

function commands(notes: readonly string[]): readonly string[] {
  return notes.filter((note) => note.startsWith("· ")).map((note) => note.slice(2).split(" ")[0] ?? "");
}

test("the listing is every command, grouped by the section the dock shows them in", () => {
  const outcome = listing(ACTIONS, STATE, "");
  const notes = outcome.notes ?? [];
  assert.equal(outcome.message, `${ACTIONS.length} commands`);
  const groups = notes.filter((note) => !note.startsWith("· "));
  assert.deepEqual(groups, [...DOCK_SECTIONS, "View"], "the dock's sections, then what the dock does not show");
});

test("every command is listed once, and nothing is listed that the registry does not hold", () => {
  const listed = commands(lines(""));
  assert.deepEqual([...listed].sort(), [...registry.actions.map((action) => action.alias)].sort());
  assert.equal(new Set(listed).size, listed.length);
});

test("each line says what the command is for, and the values it takes", () => {
  for (const note of lines("")) {
    if (note.startsWith("· ")) assert.match(note, /^· \S+( <\S+>)* — .+$/, note);
  }
});

test("`help <command>` gives the values that command takes, and no others", () => {
  assert.deepEqual(lines("theme"), ["name: dark, light"]);
  assert.deepEqual(lines("synthetic"), [
    "nodes: a whole number in 2..50000",
    "degree: a whole number in 0..12",
    "seed: a whole number in 0..4294967295",
    "shape: vault, random",
  ]);
  assert.deepEqual(lines("fit"), []);
});

test("a command is named by its word or by its id, and anything else is refused", () => {
  assert.deepEqual(lines("appearance.theme"), lines("theme"));
  assert.throws(() => listing(ACTIONS, STATE, "nope"), /`nope` is not a command/);
});
