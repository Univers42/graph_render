// The search: what it offers, and in what order, without sorting the whole node list.
import assert from "node:assert/strict";
import { test } from "node:test";

import { matchesOf } from "../src/ui/matches.ts";

const LABELS = ["Alpha", "beta", "Alphabet", "Gamma", "alphabet soup", "delta"];

test("what starts with the text comes before what holds it, each in index order", () => {
  assert.deepEqual(matchesOf(LABELS, "alpha"), [0, 2, 4]);
  assert.deepEqual(matchesOf(LABELS, "ALPHA"), [0, 2, 4]);
  assert.deepEqual(matchesOf(LABELS, "a"), [0, 2, 4, 1, 3, 5]);
});

test("an empty text matches nothing rather than everything", () => {
  assert.deepEqual(matchesOf(LABELS, ""), []);
});

test("a limit keeps the starts before the rest", () => {
  assert.deepEqual(matchesOf(LABELS, "a", 2), [0, 2]);
  assert.deepEqual(matchesOf(LABELS, "a", 0), []);
  assert.deepEqual(matchesOf(LABELS, "z", 8), []);
});

test("eight are offered by default", () => {
  const many = Array.from({ length: 30 }, (_, i) => `node ${i}`);
  assert.equal(matchesOf(many, "node").length, 8);
});
