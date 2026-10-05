import assert from "node:assert/strict";
import { test } from "node:test";
import { graphWidth } from "../graph-width.mjs";

const H = (c) => c.repeat(40);

test("a linear history is one column wide", () => {
  const graph = [`* ${H("a")}`, `* ${H("b")}`, `* ${H("c")}`].join("\n");
  assert.equal(graphWidth(graph), 1);
});

test("a branch and its merge take two columns", () => {
  // `git log --graph` prints one graph column per commit lane, two characters each.
  const graph = [
    `*   ${H("d")}`,
    `|\\`,
    `| * ${H("c")}`,
    `|/`,
    `* ${H("b")}`,
  ].join("\n");
  assert.equal(graphWidth(graph), 2);
});

test("a graph with no commit marker is zero columns wide", () => {
  assert.equal(graphWidth(""), 0);
  assert.equal(graphWidth(`|\\  ${H("a")}\n|/  ${H("b")}`), 0);
});