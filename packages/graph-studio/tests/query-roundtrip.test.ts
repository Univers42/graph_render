import assert from "node:assert/strict";
import { test } from "node:test";

import { type Query, type QueryField, parseQuery, printQuery } from "../src/console/parse.ts";

/**
 * The printer and the parser have to agree in both directions: whatever tree a search, a filter
 * or a group holds, printing it and reading it back gives that same tree. The trees are drawn
 * from a seeded PRNG, so a red case replays from its seed and nothing here needs a dependency.
 */

const CASES = 400;
const SEED = 0x5eed1234;
/** A second seed for the other direction, so a red line in one does not mask a red line in the other. */
const LINE_SEED = 0x9e3779b9;

function prng(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let mixed = state;
    mixed = Math.imul(mixed ^ (mixed >>> 15), mixed | 1);
    mixed ^= mixed + Math.imul(mixed ^ (mixed >>> 7), mixed | 61);
    return ((mixed ^ (mixed >>> 14)) >>> 0) / 4294967296;
  };
}

interface Draw {
  readonly next: () => number;
  /** True with the given chance, so a shape is a share of the cases rather than a count. */
  readonly often: (share: number) => boolean;
  readonly pick: <T>(items: readonly T[]) => T;
}

const WORDS: readonly string[] = [
  "a", "note", "two words", "", 'say "hi"', "a#b", "x:y", "a)b", "and", "OR", "a\\b", "3", "-1", "a=b",
];
const TAGS: readonly string[] = ["#x", "x", "#two words", "#", "#a#b"];
const NUMBERS: readonly string[] = ["0", "3", "12"];
const PLAIN: readonly QueryField[] = ["id", "kind", "db", "path"];
const OPS: readonly string[] = ["", "=", "!=", ">", "<"];

function makeDraw(next: () => number): Draw {
  return {
    next,
    often: (share: number) => next() < share,
    pick: <T,>(items: readonly T[]): T => {
      const item = items[Math.floor(next() * items.length) % items.length];
      if (item === undefined) throw new Error("pick from an empty list");
      return item;
    },
  };
}

function leaf(draw: Draw): Query {
  if (draw.often(0.4)) return { kind: "text", text: draw.pick(WORDS) };
  if (draw.often(0.3)) return { kind: "field", field: "tag", op: draw.pick(["", "="]), value: draw.pick(TAGS) };
  if (draw.often(0.4)) {
    return { kind: "field", field: "degree", op: draw.pick(["=", "<", ">", "<=", ">="]), value: draw.pick(NUMBERS) };
  }
  return { kind: "field", field: draw.pick(PLAIN), op: draw.pick(OPS), value: draw.pick(WORDS) };
}

/** A join the parser gives back whole: never one child, and never a join inside its own join. */
function joined(draw: Draw, depth: number, own: "and" | "or"): Query {
  const other = own === "and" ? "or" : "and";
  const parts: Query[] = [];
  const count = draw.often(0.5) ? 3 : 2;
  for (let at = 0; at < count; at += 1) {
    parts.push(draw.often(0.3) && depth > 0 ? joined(draw, depth - 1, other) : leaf(draw));
  }
  return { kind: own, of: parts };
}

function tree(draw: Draw, depth: number): Query {
  if (draw.often(0.3)) return leaf(draw);
  if (draw.often(0.3)) return { kind: "not", of: tree(draw, depth - 1) };
  if (depth <= 0) return leaf(draw);
  return joined(draw, depth - 1, draw.often(0.5) ? "and" : "or");
}

function trees(seed: number): readonly Query[] {
  const draw = makeDraw(prng(seed));
  return Array.from({ length: CASES }, () => tree(draw, 3));
}

function collect(query: Query, seen: Set<string>): void {
  seen.add(query.kind);
  if (query.kind === "not") collect(query.of, seen);
  if (query.kind === "and" || query.kind === "or") for (const part of query.of) collect(part, seen);
}

test("every printed tree reads back as the same tree", () => {
  const cases = trees(SEED);
  for (const query of cases) {
    const line = printQuery(query);
    assert.doesNotThrow(() => parseQuery(line), line);
    assert.deepEqual(parseQuery(line), query, line);
  }
  assert.equal(cases.length, CASES);
});

const LITERALS: readonly string[] = [
  "a", "note", '"two words"', '""', '"say \\"hi\\""', '"x:y"', '"a#b"', '"and"', "tag:#x", "tag:#", "tag:x",
  "kind:note", 'kind:"two words"', "id:42", "db:blog", "path:src/a.ts", "degree:>3", "degree:<=2", "a\\b",
];

function written(draw: Draw, depth: number): string {
  if (draw.often(0.3)) return draw.pick(LITERALS);
  if (draw.often(0.3)) return `NOT ${written(draw, depth - 1)}`;
  if (depth <= 0) return draw.pick(LITERALS);
  const count = draw.often(0.5) ? 3 : 2;
  const parts: string[] = [];
  for (let at = 0; at < count; at += 1) parts.push(written(draw, depth - 1));
  const joined = parts.join(draw.often(0.5) ? " AND " : " OR ");
  return draw.often(0.4) ? `(${joined})` : joined;
}

function lines(seed: number): readonly string[] {
  const draw = makeDraw(prng(seed));
  return Array.from({ length: CASES }, () => written(draw, 3));
}

test("every written query prints to a line that parses back to the same tree", () => {
  const cases = lines(LINE_SEED);
  for (const line of cases) {
    const once = parseQuery(line);
    assert.deepEqual(parseQuery(printQuery(once)), once, line);
  }
  assert.equal(cases.length, CASES);
});

test("the seed drew every kind of node, and enough groups to need parentheses", () => {
  const cases = trees(SEED);
  const seen = new Set<string>();
  for (const query of cases) collect(query, seen);
  assert.deepEqual([...seen].sort(), ["and", "field", "not", "or", "text"]);
  const grouped = cases.filter((query) => printQuery(query).includes("("));
  assert.ok(grouped.length > 100, `only ${grouped.length} of ${cases.length} cases printed a group`);
});

test("the generated cases are the ones the count claims", () => {
  assert.equal(trees(SEED).length, CASES);
  assert.equal(lines(LINE_SEED).length, CASES);
  assert.notDeepEqual(trees(SEED), trees(SEED + 1));
});
