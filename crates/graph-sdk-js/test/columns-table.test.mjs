// `Table` — the string table both columnar encoders intern through — over its own contract,
// with no module and no wasm.
//
// What it pins:
//   * the field path a refusal names is `section[row].column`, built from a section and a column
//     the caller passes **as constants** — so a batch of 10 000 nodes allocates no field paths at
//     all (`docs/measurements/perf-p4f-wasm.md` measured `intern` at ~14 % of the wasm32
//     `extend` timer, most of it this string);
//   * the index a value gets is its first-seen position, and a repeat is the same index (D4);
//   * a lone surrogate is refused by `intern` itself, named by that path.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";

import { ColumnsEncoderError } from "../src/columns-assemble.ts";
import { Table } from "../src/columns.ts";

test("a refusal names section[row].column, from the constants the caller passes", () => {
  const table = new Table();
  assert.throws(
    () => table.intern("edges", "target", 3, "no\ud800where"),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "edges[3].target");
      assert.match(error.message, /not well-formed/);
      return true;
    },
  );
});

test("an absent optional interns nothing and reads back as ABSENT", () => {
  const table = new Table();
  assert.equal(table.optional("nodes", "icon", 0, null), 0xffff_ffff);
  assert.equal(table.strings.length, 0, "and the table did not grow");
});

test("a value's index is its first-seen position, and a repeat is that same index", () => {
  const table = new Table();
  assert.equal(table.intern("nodes", "kind", 0, "record"), 0);
  assert.equal(table.intern("nodes", "kind", 1, "note"), 1);
  // The repeat is the whole reason the table exists: the string table is a pure function of the
  // document's *first sighting* order, so this must not append a second "record".
  assert.equal(table.intern("nodes", "kind", 2, "record"), 0);
  assert.deepEqual(table.strings, ["record", "note"]);
});

test("the strings handed out are the values handed in, in index order", () => {
  const table = new Table();
  ["z", "a", "m"].forEach((value, row) => table.intern("nodes", "id", row, value));
  assert.deepEqual(table.strings, ["z", "a", "m"], "not sorted: insertion order is the bytes");
  assert.equal(table.index.get("a"), 1);
});
