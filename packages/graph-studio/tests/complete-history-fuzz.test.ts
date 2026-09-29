/**
 * Seeded property rows for what Tab offers and what the console remembers, beside the
 * hand-written rows in console.test.ts and ui-history.test.ts.
 *
 * Ponytail: prefixes are joined from the studio's own aliases, parameter names and value
 * fragments (the tables below), so the corpus holds no word the console could not produce
 * and no free text beyond the one space-bearing fragment — the mis-completed quoted value
 * that complete() documents (its own Ponytail, complete.ts:6) is only in reach because VALUES
 * carries "graph notes". The history rows check the bound, the order and the cursor, never that
 * a kept line is one a user would have wanted remembered. Escape hatch: add a fragment to a
 * table and re-run the same seed — a new fragment is a new corpus, not a new seed.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Action, createRegistry } from "../src/actions/registry.ts";
import { complete } from "../src/console/complete.ts";
import { type History, EMPTY_HISTORY, pushLine } from "../src/ui/history.ts";
import { stream } from "./fuzz.ts";

const SEED = 0x5eed17;
const DRAWS = 10_000;
const LIMIT = 100;

interface State {
  readonly layouts: readonly string[];
  readonly themes: readonly string[];
}

const STATE: State = {
  layouts: ["layout.grid", "layout.force.barnes_hut", "layout.forceatlas2", "layout.none"],
  themes: ["theme.dark", "theme.light"],
};

const ACTIONS: readonly Action<State, null>[] = [
  {
    id: "layout.run", alias: "layout", title: "Run a layout", section: "Layout",
    params: [{ name: "id", kind: "choice", title: "Layout", choices: (state) => state.layouts, value: () => "layout.grid" }],
    run: () => ({ message: "" }),
  },
  {
    id: "source.synthetic", alias: "synthetic", title: "Generate", section: "Source",
    params: [
      { name: "nodes", kind: "int", title: "Nodes", min: 2, max: 100_000, value: () => 120 },
      { name: "seed", kind: "int", title: "Seed", value: () => 0 },
    ],
    run: () => ({ message: "" }),
  },
  {
    id: "filter.text", alias: "filter", title: "Name contains", section: "Filters",
    params: [{ name: "text", kind: "text", title: "Name contains", value: () => "" }],
    run: () => ({ message: "" }),
  },
  { id: "view.fit", alias: "fit", title: "Fit the graph to the view", section: null, params: [], run: () => ({ message: "" }) },
  { id: "console.clear", alias: "clear", title: "Empty the console", section: null, params: [], run: () => ({ message: "" }) },
  {
    id: "appearance.theme", alias: "theme", title: "Theme", section: "Appearance",
    params: [{ name: "name", kind: "choice", title: "Theme", choices: (state) => state.themes, value: () => "theme.dark" }],
    run: () => ({ message: "" }),
  },
];

const registry = createRegistry(ACTIONS);

/** Real aliases, so `registry.find` resolves them: the first word is the command. */
const KNOWN: readonly string[] = ["layout", "synthetic", "filter", "fit", "clear", "theme"];
/** Half-typed aliases: the first word is a command that does not exist yet. */
const PARTIALS: readonly string[] = ["s", "l", "f", "t", "cl", "th", ""];
const UNKNOWN: readonly string[] = ["nope", "layuot", "layouts", "z", "La", "thme", "clearx", "x"];
const VALUES: readonly string[] = [
  "", "g", "gr", "grid", "force", "forceatlas2", "layout.", "layout.g", "atlas2", "none", "0", "120", "-3",
  "e", "l", "f", "c", "t", "d", "dark", "light", "theme.d", "=a", "a=b", "😀", "é", "\t", "\\", "'", "graph notes",
];
const NAMES: readonly string[] = ["id", "text", "nodes", "seed", "name", "by", "mode", "nope", "id=x", "="];

/** A line a user could be part-way through typing: an alias, then a name, then a value. */
function prefix(alias: string, name: string, value: string, word: number): string {
  if (word === 0) return alias;
  if (word === 1) return `${alias} `;
  if (word === 2) return `${alias} ${name}`;
  if (word === 3) return `${alias} ${name}=`;
  return `${alias} ${name}=${value}`;
}

/**
 * What a completion must not touch: the command word, the parameter name, and the `=`.
 * The value after that is the word being completed and is free to change — that is the
 * whole point of completing it. Read the way `offer` reads it (complete.ts:55): leading
 * whitespace is dropped and the last whitespace-separated word is the value, so a value
 * holding a space is completed as its last word alone — the split complete.ts:6 documents.
 */
function offered(typed: string): { readonly head: string; readonly value: string } {
  const line = typed.trimStart();
  const last = line.split(/\s+/).pop() ?? "";
  const value = last.slice(last.indexOf("=") + 1);
  return { head: line.slice(0, line.length - value.length), value };
}

test(`a completion keeps the line before the value it completes, over ${DRAWS} seeded prefixes`, (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS} (known commands only; unknown ones are their own row)`);
  const draws = stream(SEED);
  for (let at = 0; at < DRAWS; at += 1) {
    const typed = prefix(draws.pick(KNOWN), draws.pick(NAMES), draws.pick(VALUES), draws.int(0, 4));
    const { text } = complete(typed, registry, STATE);
    const { head: kept } = offered(typed);
    assert.ok(text.startsWith(kept),
      `draw ${at} at seed ${SEED}: ${JSON.stringify(typed)} completed to ${JSON.stringify(text)}, ` +
      `which does not start with ${JSON.stringify(kept)}`);
  }
});

test("a completion of a command the studio does not have is the line, unchanged", (t) => {
  t.diagnostic(`seed=${SEED} draws=2000`);
  // `offer` gives an unknown word the whole text as its head and no candidates
  // (complete.ts:64, `head: text` un-trimmed), so the line comes back exactly as it went in.
  // Not covered above: there the head is read off the trimmed line, and this path is not.
  const draws = stream(SEED);
  for (let at = 0; at < 2000; at += 1) {
    const typed = prefix(draws.pick(UNKNOWN), draws.pick(NAMES), draws.pick(VALUES), draws.int(0, 4));
    const { text, candidates } = complete(typed, registry, STATE);
    assert.deepEqual(candidates, [], `${JSON.stringify(typed)} offered ${JSON.stringify(candidates)}`);
    assert.equal(text, typed, `${JSON.stringify(typed)} came back as ${JSON.stringify(text)}`);
  }
});

test("every candidate offered contains the value that was typed", (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS}`);
  const draws = stream(SEED);
  for (let at = 0; at < DRAWS; at += 1) {
    const typed = prefix(draws.pick(KNOWN), draws.pick(NAMES), draws.pick(VALUES), draws.int(0, 4));
    const { candidates } = complete(typed, registry, STATE);
    const wanted = offered(typed).value.toLowerCase();
    for (const candidate of candidates) {
      assert.ok(candidate.toLowerCase().includes(wanted),
        `draw ${at} at seed ${SEED}: ${JSON.stringify(candidate)} does not contain ${JSON.stringify(wanted)} ` +
        `from ${JSON.stringify(typed)}`);
    }
  }
});

test("a single candidate replaces the value rather than extending it — the counterexample", (t) => {
  t.diagnostic(`seed=${SEED}`);
  // This is the row the seeded property above cannot contain: with exactly one candidate,
  // `complete` returns the whole choice, so the result does NOT start with what was typed
  // (`extend`, complete.ts:37). console.test.ts:95 already pins it for `layout atlas`.
  const cases: readonly (readonly [string, string])[] = [
    ["layout atlas", "layout layout.forceatlas2 "],
    ["layout grid", "layout layout.grid "],
    ["layout barnes", "layout layout.force.barnes_hut "],
    ["theme name=d", "theme name=theme.dark "],
  ];
  for (const [typed, done] of cases) {
    assert.equal(complete(typed, registry, STATE).text, done);
    assert.ok(!done.startsWith(typed), `${JSON.stringify(typed)} unexpectedly extends: ${JSON.stringify(done)}`);
  }
});

test("a single word with no space in it is completed to a command that starts with it", (t) => {
  t.diagnostic(`seed=${SEED} draws=2000`);
  // No space, or the line is two words and the second is a value: `complete` reads the last
  // word as the value, not the first as a partial command.
  const draws = stream(SEED);
  for (let at = 0; at < 2000; at += 1) {
    const typed = `${draws.pick(KNOWN)}${draws.pick(VALUES).replace(/\s+/g, "")}${draws.pick(PARTIALS)}`;
    const { text, candidates } = complete(typed, registry, STATE);
    if (candidates.length === 0) {
      assert.equal(text, typed, `${JSON.stringify(typed)} was rewritten with nothing offered`);
      continue;
    }
    for (const candidate of candidates) {
      assert.ok(candidate.startsWith(typed),
        `${JSON.stringify(candidate)} does not start with ${JSON.stringify(typed)}`);
    }
  }
});

test("a leading space is dropped: the completion is offered for the trimmed line", (t) => {
  t.diagnostic(`seed=${SEED}`);
  // `offer` trims the start (complete.ts:56) and `Console.tsx:89` writes `done.text` straight
  // back into the input, so the space the user typed is gone. This is a finding, not a
  // contract: the value returned is the completed line, not one prefixed by what was typed.
  for (const typed of [" la", "  la", "\tla"]) {
    const { text } = complete(typed, registry, STATE);
    assert.equal(text, "layout ", `${JSON.stringify(typed)} completed to ${JSON.stringify(text)}`);
    assert.ok(!text.startsWith(typed), `${JSON.stringify(typed)} kept its leading space: ${JSON.stringify(text)}`);
  }
});

/** 1000 lines through the real `pushLine`, with a repeat and a blank every third push. */
function pushThousand(): { readonly history: History; readonly typed: readonly string[] } {
  const typed: string[] = [];
  let history: History = EMPTY_HISTORY;
  for (let at = 0; at < 1000; at += 1) {
    const line = at % 3 === 0 ? `run ${at}` : at % 3 === 1 ? `run ${at - 1}` : "   ";
    typed.push(line);
    history = pushLine(history, line);
  }
  return { history, typed };
}

test(`history holds at most ${LIMIT} lines through 1000 pushes, and keeps the last ones`, (t) => {
  t.diagnostic(`seed=${SEED} pushes=1000 limit=${LIMIT}`);
  const draws = stream(SEED);
  const typed: string[] = [];
  let history: History = EMPTY_HISTORY;
  for (let at = 0; at < 1000; at += 1) {
    const line = `run ${at} ${draws.pick(VALUES)}`;
    typed.push(line);
    history = pushLine(history, line);
    assert.ok(history.lines.length <= LIMIT,
      `after ${at + 1} pushes the history holds ${history.lines.length}, over the limit of ${LIMIT}`);
  }
  assert.equal(history.lines.length, LIMIT);
  assert.equal(history.at, LIMIT);
  // Not `typed.slice(-LIMIT)`: `pushLine` stores the line TRIMMED, so a value that ends the
  // line in a space is remembered without it, and the kept set is the last 100 of the
  // trimmed non-blank lines. The row above already checked the bound on every push.
  assert.deepEqual(history.lines, typed.map((line) => line.trim()).slice(-LIMIT));
});

test("the bound holds across a thousand pushes with repeats and blanks on the path", (t) => {
  t.diagnostic(`pushes=1000 (deterministic: every third line repeats or is blank)`);
  const { history, typed } = pushThousand();
  assert.ok(history.lines.length <= LIMIT);
  assert.equal(history.at, history.lines.length);
  for (let at = 1; at < history.lines.length; at += 1) {
    const earlier = typed.lastIndexOf(history.lines[at - 1] ?? "");
    const later = typed.lastIndexOf(history.lines[at] ?? "");
    assert.ok(earlier < later, `lines ${at - 1} and ${at} are not in the order they were typed`);
  }
});

test("a repeated line and a blank line never grow the history", (t) => {
  t.diagnostic(`pushes=1000 (deterministic)`);
  const one = pushLine(EMPTY_HISTORY, "layout grid");
  assert.deepEqual(pushLine(one, "layout grid"), one, "a repeated line must not be kept twice");
  assert.deepEqual(pushLine(one, "   \t\n"), one, "a blank line must not be kept");
  let history = one;
  for (let at = 0; at < 1000; at += 1) history = pushLine(history, "layout grid");
  assert.equal(history.lines.length, 1);
});
