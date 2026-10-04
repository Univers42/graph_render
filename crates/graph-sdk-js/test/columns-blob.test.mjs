// The blob-placement arithmetic of `assembleColumns`, over hand-built rows: which path a string
// table takes, what the offsets say, and what the buffer is sized to.
//
// What it pins:
//   * the table's total UTF-8 length decides the offsets, and the buffer is sized to it — never
//     grown, because the decoder refuses a document whose sections do not sum to its length;
//   * an all-ASCII table is placed by **one** `encodeInto` over the joined text, and a table with
//     any code point wider than a byte is placed string by string, each into a slot its own
//     offsets name;
//   * a non-ASCII table is never joined. `docs/measurements/perf-p4f-wasm.md` measured the join
//     as pure waste on the stream the studio generates (its icons are `🌿` and `📈`, so the exact
//     path is the one that runs, and the joined copy was allocated and thrown away);
//   * the two paths produce the **same bytes** for the same table, which is what makes the first
//     bullet a claim about the format rather than about one code path.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";

import { assembleColumns } from "../src/index.ts";
import { utf8Length } from "../src/columns-assemble.ts";

import { ABSENT, rows } from "./columns-fixtures.mjs";

/** The offsets table's `string_count + 1` words, read out of a built document. */
function offsetsOf(bytes, stringCount) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  return Array.from({ length: stringCount + 1 }, (_, i) => view.getUint32(32 + 4 * i, true));
}

/** A one-node, one-edge document over `table`, so the cells never matter to this file. */
function document(table) {
  return rows(
    table,
    [[0], [1], [ABSENT], [2], [3], [ABSENT], [ABSENT], [0]],
    [[4], [0], [0], [5], [5], [ABSENT], [0], [0]],
    [[1], [0], [0.5]],
  );
}

test("the offsets are the running UTF-8 sums, and the header declares the blob they end on", () => {
  const table = ["a", "é", "b", "🌿", "c"];
  const bytes = assembleColumns(document(table));
  const offsets = offsetsOf(bytes, table.length);
  const running = table.reduce((sum, value) => sum + utf8Length(value), 0);
  assert.deepEqual(offsets, [0, 1, 3, 4, 8, 9], `the sums of ${JSON.stringify(table)}`);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  assert.equal(view.getUint32(20, true), running, "the header's blob length is the last offset");
  assert.equal(view.getUint32(0, true), 0x3143_4d47, "and this is a GMC1 document, as `rows` builds");
});

test("a table of ASCII and one with a wide code point place their entries the same way", () => {
  // The same table twice over, the difference one `ć`: one code unit, two bytes. Both must place
  // their entries identically relative to their own widths, or a document would read back
  // differently depending on which path the producer's data happened to take.
  const narrow = ["n-0", "record", "db-0", "note"];
  const wide = ["n-0", "rećord", "db-0", "note"];
  const narrowBytes = assembleColumns(document(narrow));
  const wideBytes = assembleColumns(document(wide));
  assert.equal(wide.length - narrow.length, 0, "one code unit either way");
  // The offsets are the claim, not the buffer's length: the blob is padded to a multiple of 8
  // before the columns start, so one extra byte of blob does not change the document's size.
  assert.deepEqual(offsetsOf(narrowBytes, 4), [0, 3, 9, 13, 17], "code-unit counts");
  assert.deepEqual(offsetsOf(wideBytes, 4), [0, 3, 10, 14, 18], "UTF-8 byte counts");
});

test("a non-ASCII table is never joined into one string", () => {
  // The regression this pins, stated as behaviour rather than as a profile: `join("")` copies the
  // whole blob into a fresh string, and the exact path — the one any table holding an emoji
  // takes — never reads it. At 1M nodes that copy is a megabyte a batch, allocated and dropped.
  const table = ["n-é", "record", "🌿", "note"];
  const joined = Array.prototype.join;
  let joins = 0;
  Array.prototype.join = function patched(...args) {
    joins += 1;
    return joined.apply(this, args);
  };
  let bytes;
  try {
    bytes = assembleColumns(document(table));
  } finally {
    Array.prototype.join = joined;
  }
  assert.ok(bytes.length > 0, "the document was written");
  assert.equal(joins, 0, "the exact path placed the table without joining it");
});

test("an ASCII table is joined once, and placed by a single encodeInto over it", () => {
  const table = ["n-0", "record", "db-0", "note"];
  const joined = Array.prototype.join;
  let joins = 0;
  Array.prototype.join = function patched(...args) {
    joins += 1;
    return joined.apply(this, args);
  };
  let bytes;
  try {
    bytes = assembleColumns(document(table));
  } finally {
    Array.prototype.join = joined;
  }
  assert.deepEqual(offsetsOf(bytes, 4), [0, 3, 9, 13, 17]);
  assert.equal(joins, 1, "one join for the whole table, then one placement");
});

test("a table with a lone surrogate is refused by index, and the refusal is not a byte count", () => {
  const table = ["n-0", "re\ud800ord", "db-0", "note"];
  assert.throws(
    () => assembleColumns(document(table)),
    (error) => {
      assert.equal(error.field, "strings[1]", "the entry that holds it, by index");
      assert.match(error.message, /not well-formed/);
      return true;
    },
  );
});
