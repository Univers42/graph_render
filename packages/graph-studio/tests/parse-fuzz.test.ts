/**
 * Seeded property rows for the console's grammar, beside the hand-written rows in
 * console.test.ts.
 *
 * Ponytail: every line is a join of CONSOLE_ALPHABET fragments (fuzz.ts), so the corpus
 * cannot contain a shape the table lacks — a 4 KB line is only reachable through
 * `longLine`, and there is exactly one of those per run, drawn from the same stream. The
 * stream cannot shrink a failure either: a red run prints the whole line, not the three
 * characters in it, so the failing input has to be cut by hand. An empty pass means "the
 * table broke nothing", not "the parser is safe". Escape hatch: add the fragment to
 * CONSOLE_ALPHABET and re-run; the same seed then draws a different corpus.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Action, ActionRefusal, type Registry, createRegistry } from "../src/actions/registry.ts";
import { CommandRefusal, parseCommand } from "../src/console/parse.ts";
import { CONSOLE_ALPHABET, line, longLine, stream } from "./fuzz.ts";

const SEED = 0x0c0ffee;
const DRAWS = 10_000;

interface State {
  readonly layouts: readonly string[];
  readonly loaded: boolean;
}

const STATE: State = {
  layouts: ["layout.grid", "layout.force.barnes_hut", "layout.forceatlas2", "layout.none"],
  loaded: true,
};

/** The studio's real vocabulary, so a draw can land on a command the console knows. */
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
      { name: "degree", kind: "int", title: "Links per node", min: 0, max: 64, value: () => 2 },
      { name: "seed", kind: "int", title: "Seed", min: 0, max: 2 ** 31, value: () => 0 },
      { name: "shape", kind: "choice", title: "Shape", choices: () => ["circle", "square", "text"], value: () => "circle" },
    ],
    run: () => ({ message: "" }),
  },
  {
    id: "filter.text", alias: "filter", title: "Name contains", section: "Filters",
    params: [{ name: "text", kind: "text", title: "Name contains", value: () => "" }],
    run: () => ({ message: "" }),
  },
  {
    id: "view.zoom", alias: "zoom", title: "Zoom by a factor", section: null,
    params: [{ name: "factor", kind: "number", title: "Factor", min: 0.02, max: 40, value: () => 1.25 }],
    run: () => ({ message: "" }),
  },
  {
    id: "view.pan", alias: "pan", title: "Pan by screen pixels", section: null,
    params: [
      { name: "dx", kind: "number", title: "Right", min: -4000, max: 4000, value: () => 0 },
      { name: "dy", kind: "number", title: "Down", min: -4000, max: 4000, value: () => 0 },
    ],
    run: () => ({ message: "" }),
  },
  { id: "view.fit", alias: "fit", title: "Fit the graph to the view", section: null, params: [], run: () => ({ message: "" }) },
  { id: "console.clear", alias: "clear", title: "Empty the console", section: null, params: [], run: () => ({ message: "" }) },
];

const registry: Registry<State, null> = createRegistry(ACTIONS);

/** `null` when the line was read; the `CommandRefusal` when it was refused. */
function outcome(text: string): CommandRefusal | null {
  try {
    parseCommand(text, registry);
    return null;
  } catch (error) {
    if (error instanceof CommandRefusal) return error;
    throw error;
  }
}

/**
 * The property: a line is either a command or a `CommandRefusal`. Anything else is a
 * defect in the parser, not in the line — a `TypeError` from a `Map` key, a `RangeError`
 * from an index, a hang. An unknown word is a refusal by design (`unknown`, parse.ts:113), and
 * a value past the last parameter is a refusal too (`readValues`, parse.ts:100), so a long
 * junk line is fine.
 */
function reads(text: string): boolean {
  return outcome(text) === null;
}

function refuses(text: string): boolean {
  return outcome(text) !== null;
}

test("the alphabet still holds every class the rows below claim to cover", (t) => {
  t.diagnostic(`fragments=${CONSOLE_ALPHABET.length}`);
  // A metatest, and deliberately so: the property above is only as good as this table, and
  // a class that quietly went missing would leave the property passing over untested input.
  const CLASSES: readonly (readonly [string, RegExp])[] = [
    ["NUL", /\0/],
    ["a control character", /[\u0000-\u001f\u007f]/],
    ["a lone surrogate", /[\ud800-\udfff]/],
    ["an astral-plane emoji", /\p{Extended_Pictographic}/u],
    ["a non-ASCII letter", /[^\u0020-\u007e]/],
    ["an equals sign", /=/],
    ["a quote", /["']/],
    ["a backslash", /\\/],
    ["a line or paragraph separator", /[\u2028\u2029]/],
    ["a byte-order mark", /\ufeff/],
  ];
  for (const [name, pattern] of CLASSES) {
    assert.ok(CONSOLE_ALPHABET.some((part) => pattern.test(part)), `no fragment in the alphabet is ${name}`);
  }
});

test("a seeded line the parser reads resolves to an action through the registry", (t) => {
  t.diagnostic(`seed=${SEED} draws=1000`);
  // The parser stopping without throwing is not the same as the line being usable: take the
  // lines it READ and put them through `resolve`, which checks the values. A raw value the
  // parser accepted and the registry then refuses is a real disagreement between the two.
  const draws = stream(SEED);
  for (let at = 0; at < 1000; at += 1) {
    const text = line(draws, 8);
    if (!reads(text)) continue;
    const { id, raw } = parseCommand(text, registry);
    let resolved;
    try {
      resolved = registry.resolve(id, raw, STATE);
    } catch (error) {
      // A refusal here is `resolve`'s job and carries its own code; it is not a crash.
      assert.ok(error instanceof ActionRefusal, `draw ${at} at seed ${SEED}: ${JSON.stringify(text)} ` +
        `resolved to ${String(error)}`);
      continue;
    }
    assert.equal(resolved.action.id, id);
  }
});

test(`the parser reads or refuses every one of ${DRAWS} seeded lines, and never throws anything else`, (t) => {
  t.diagnostic(`seed=${SEED} draws=${DRAWS}`);
  const draws = stream(SEED);
  for (let at = 0; at < DRAWS; at += 1) {
    const text = line(draws, 12);
    let thrown: unknown;
    try {
      parseCommand(text, registry);
    } catch (error) {
      thrown = error;
    }
    assert.ok(
      thrown === undefined || thrown instanceof CommandRefusal,
      `draw ${at} at seed ${SEED} raised ${String(thrown)}: ${JSON.stringify(text)}`,
    );
  }
});

test("the same property over the classes the table promises: unicode, controls, surrogates, emoji, NUL", (t) => {
  t.diagnostic(`seed=${SEED}`);
  const CLASSES: readonly (readonly [string, string])[] = [
    ["a NUL byte", "\0layout"],
    ["a control character", "layout\u0007"],
    ["a lone high surrogate", "layout \ud800"],
    ["a lone low surrogate", "layout \udc00"],
    ["an emoji", "layout 😀"],
    ["a right-to-left letter", "layout م"],
    ["a line separator", "layout\u2028grid"],
    ["only whitespace", "   \t\n"],
    ["an unclosed quote", 'layout "grid'],
    ["an unclosed single quote", "layout 'grid"],
    ["a backslash at the end", "layout \\"],
    ["a name given with no value", "layout id="],
    ["a value with an equals in it", "filter text=a=b"],
    ["a bare equals", "="],
  ];
  for (const [name, text] of CLASSES) {
    assert.doesNotThrow(() => outcome(text), `${name}: ${JSON.stringify(text)} was neither read nor refused`);
  }
});

test("a line with no command word at all is refused, not read", (t) => {
  t.diagnostic(`seed=${SEED}`);
  for (const text of ["", " ", "\t", "   \t\n", "=", "==", "'", '"', "\\", "😀", "é"]) {
    assert.ok(refuses(text), `${JSON.stringify(text)} must be refused, not read`);
  }
});

test("a 4 KB line, and a 4 KB unclosed quote, are read or refused like anything else", (t) => {
  t.diagnostic(`seed=${SEED}`);
  const draws = stream(SEED ^ 0x4000);
  const cases = [longLine(draws, 4096), `${longLine(draws, 4096)}"`];
  const started = process.hrtime.bigint();
  for (const text of cases) {
    assert.doesNotThrow(() => outcome(text), `a 4 KB line raised something other than a CommandRefusal`);
  }
  t.diagnostic(`${cases.length} 4 KB lines cost ${process.hrtime.bigint() - started} ns in total`);
});

test("a lone surrogate is kept in the value rather than dropped or doubled", (t) => {
  t.diagnostic(`seed=${SEED}`);
  const text = `filter text=a\ud800b`;
  assert.deepEqual(parseCommand(text, registry), { id: "filter.text", raw: { text: "a\ud800b" } });
});

test("a command word is matched by its alias, not by a prefix of a longer one", (t) => {
  t.diagnostic(`seed=${SEED}`);
  assert.deepEqual(parseCommand("layout", registry), { id: "layout.run", raw: {} });
  assert.deepEqual(parseCommand("layout.run", registry), { id: "layout.run", raw: {} });
  assert.ok(refuses("layouts"), "`layouts` must be refused, not read as `layout`");
  assert.ok(refuses("layou"), "`layou` must be refused, not guessed at");
  // `layout` takes one value, so a word after it is that value, not a second command.
  assert.deepEqual(parseCommand("layout extra", registry), { id: "layout.run", raw: { id: "extra" } });
  assert.ok(refuses("fit extra"), "a command taking nothing is refused an extra value");
  // A text parameter takes anything as its ONE value, but a second word is read as the next
  // parameter and there is none: `filter text=a b` is `text` given twice, not `text` = "a b".
  // Quotes are how a value holds a space (`quoted`, parse.ts:127).
  assert.ok(reads('filter "a b c"'), "a quoted value holds spaces");
  assert.deepEqual(parseCommand('filter "a b c"', registry), { id: "filter.text", raw: { text: "a b c" } });
  assert.ok(refuses("filter text=a b"), "a second bare word is not appended to a named value");
});
