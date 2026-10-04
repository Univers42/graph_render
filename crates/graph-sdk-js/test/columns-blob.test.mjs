// The blob-placement arithmetic of `assembleColumns`, over hand-built rows: which path a string
// table takes, what the offsets say, and what the buffer is sized to.
//
// What it pins:
//   * the table's total UTF-8 length decides the offsets, and the buffer is sized to it — never
//     grown, because the decoder refuses a document whose sections do not sum to its length;
//   * an all-ASCII table and one with any code point wider than a byte place their entries
//     identically relative to their own widths, and **both** are placed by a single
//     `encodeInto` over one join — the table is never placed entry by entry, which cost a
//     `subarray` object per entry (`docs/measurements/perf-p4g-wasm.md`);
//   * the two paths produce the **same bytes** for the same table, which is what makes the first
//     bullet a claim about the format rather than about one code path.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { TextEncoder } from "node:util";

import { assembleColumns } from "../src/index.ts";
import { blobAt } from "../src/columns-blob.ts";
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

test("a non-ASCII table is placed by one encodeInto over the join, never entry by entry", () => {
  // The arrangement this pins as behaviour. Placing entry by entry meant a `subarray` object
  // per entry — 160 000 throwaway views a batch — which `docs/measurements/perf-p4g-wasm.md`
  // measures at 9.76 ms against this form's 3.80 ms. `perf-p4f-wasm.md` had removed a join
  // whose product was *discarded*; this counts placements, so the copy is counted as one that
  // is read and a regression to per-entry views is a failure here rather than a profile.
  const table = ["n-é", "record", "🌿", "note"];
  const joined = Array.prototype.join;
  const encodeInto = TextEncoder.prototype.encodeInto;
  let joins = 0;
  let placements = 0;
  Array.prototype.join = function patched(...args) {
    joins += 1;
    return joined.apply(this, args);
  };
  TextEncoder.prototype.encodeInto = function counted(...args) {
    placements += 1;
    return encodeInto.apply(this, args);
  };
  let bytes;
  try {
    bytes = assembleColumns(document(table));
  } finally {
    Array.prototype.join = joined;
    TextEncoder.prototype.encodeInto = encodeInto;
  }
  assert.ok(bytes.length > 0, "the document was written");
  assert.equal(joins, 1, "the table is joined once");
  assert.equal(placements, 1, "and placed by a single encodeInto, not one per entry");
  // And the bytes are the entries' — one placement must not be one entry's worth of them.
  const at = blobAt(table.length);
  const total = table.map((v) => utf8Length(v)).reduce((a, b) => a + b, 0);
  assert.equal(Buffer.from(bytes.subarray(at, at + total)).toString("utf8"), table.join(""));
});

test("an ASCII table is joined once, and placed by a single encodeInto over it", () => {
  const table = ["n-0", "record", "db-0", "note"];
  const joined = Array.prototype.join;
  const encodeInto = TextEncoder.prototype.encodeInto;
  let joins = 0;
  let placements = 0;
  Array.prototype.join = function patched(...args) {
    joins += 1;
    return joined.apply(this, args);
  };
  TextEncoder.prototype.encodeInto = function counted(...args) {
    placements += 1;
    return encodeInto.apply(this, args);
  };
  let bytes;
  try {
    bytes = assembleColumns(document(table));
  } finally {
    Array.prototype.join = joined;
    TextEncoder.prototype.encodeInto = encodeInto;
  }
  assert.deepEqual(offsetsOf(bytes, 4), [0, 3, 9, 13, 17]);
  assert.equal(joins, 1, "one join for the whole table");
  assert.equal(placements, 1, "then one placement of it");
});

test("an emoji beside a 2-byte and a 3-byte character: the widths are the sums, and the blob is theirs", () => {
  // The three width classes inside one entry and beside each other across entries — the case
  // a code-unit walk and a code-point walk part company on. `é` is two bytes, `€` three, `🌿`
  // four, `𝄞` (a supplementary plane character, two code units) four, and `￿` (U+FFFF, one
  // code unit, three bytes) three: so an entry's code-unit count and its byte count differ by
  // a different amount on each row of this table.
  const table = ["🌿é", "€", "a", "🌿🌿", "é€", "𝄞", "￿"];
  const widths = table.map((value) => utf8Length(value));
  assert.deepEqual(widths, [6, 3, 1, 8, 5, 4, 3], `the byte widths of ${JSON.stringify(table)}`);
  const bytes = assembleColumns(document(table));
  const total = widths.reduce((a, b) => a + b, 0);
  assert.deepEqual(
    offsetsOf(bytes, table.length),
    [0, 6, 9, 10, 18, 23, 27, 30],
    "the offsets are the running sums",
  );
  // And the blob is exactly the entries' encodings, so the decoder hands back the table: with
  // the table placed by one `encodeInto` over the join, a skipped low surrogate or a miscounted
  // class would move a byte and fail here rather than in a profile.
  const at = blobAt(table.length);
  assert.equal(Buffer.from(bytes.subarray(at, at + total)).toString("utf8"), table.join(""));
  // The header's blob length is the same total, from the same widths, on the same pass.
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  assert.equal(view.getUint32(20, true), total);
});

test("a lone surrogate measures three bytes, the way the code-point walk measured it", () => {
  // `exactWidth` refuses a lone surrogate before `utf8Length` ever sees one, so this is the
  // exported function's own contract: the same number for every input, well-formed or not.
  assert.equal(utf8Length("a\ud800b"), 5, "two ASCII and one lone surrogate between them");
  assert.equal(utf8Length("\udc00"), 3, "a lone low surrogate");
  assert.equal(utf8Length("\udc00\udc00"), 6, "a low surrogate is never the high half of a pair");
  assert.equal(utf8Length("\ud800"), 3, "a lone high surrogate at the end");
  assert.equal(utf8Length("🌿"), 4, "a whole pair is four");
  assert.equal(utf8Length(""), 0);
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
