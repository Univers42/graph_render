// `assembleColumns` — the encoder for a producer that already holds columns — over the
// real release artifact.
//
// What it pins, in order of how badly a regression would hurt:
//   * `assembleColumns` produces byte-identical output to `encodeColumns`, takes the ASCII
//     fast path, falls to the exact path for a multi-byte table, and refuses what
//     `encodeColumns` refuses.
//
// Run: `node --test --experimental-strip-types crates/graph-sdk-js/test/`.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  assembleColumns,
  ColumnsEncoderError,
  GraphMotorError,
  encodeColumns,
} from "../src/index.ts";

import {
  ABSENT,
  asciiDocument,
  asciiRows,
  idsOf,
  motor,
  rows,
} from "./columns-fixtures.mjs";

test("assembleColumns and encodeColumns give the same bytes for the same document", () => {
  const built = assembleColumns(asciiRows());
  const interned = encodeColumns(asciiDocument());
  assert.ok(built.length > 0, "the assembler wrote a document");
  assert.equal(built.length, interned.length, "the same sections, so the same length");
  assert.ok(Buffer.from(built).equals(Buffer.from(interned)), "identical bytes");
});

test("a table with duplicate entries builds, and each row keeps its own id", async () => {
  const m = await motor();
  // "n-0", "n-1" and "e-0" each appear twice: `docs/contract/ingest-columns.md` says two
  // entries may hold the same bytes and the arena interns by content, so the repeats cost
  // nothing downstream. The dense-row rule is the other half — the *ids* stay unique, or the
  // motor refuses the document as ColumnsInvalid.
  const repeated = assembleColumns(rows(
    ["n-0", "record", "db-0", "studio", "Graph notes", "n-0", "n-1", "note", "", "n-2",
      "n-1", "notes", "e-0", "relation", "e-0", "e-1", "hierarchy"],
    [
      [0, 6, 9], [1, 7, 1], [2, ABSENT, 2], [3, 3, 3], [4, 8, 11],
      [ABSENT, ABSENT, ABSENT], [ABSENT, ABSENT, ABSENT], [1, 0, 0],
    ],
    [
      [12, 15], [0, 1], [1, 2], [13, 16], [13, 16], [ABSENT, ABSENT], [0, 1], [0, 0],
    ],
    [[1, 0.5, 0.25], [0, 0, 0], [0.5, 0.5]],
  ));
  const handle = m.buildColumns(repeated);
  assert.equal(m.nodeCount(handle), 3);
  assert.deepEqual(idsOf(m, handle), ["n-0", "n-1", "n-2"]);
});

test("a multi-byte string takes the exact path, builds, and its id reads back the same", async () => {
  const m = await motor();
  // "é" is one code unit and two UTF-8 bytes, which is what makes the ASCII fast path miss:
  // a buffer sized at one byte per code unit cannot hold it.
  const table = ["n-é", "record", "db-0", "studio", "Graph notes", "n-1", "note", "notes é",
    "n-2", "notes", "e-0", "relation", "e-1", "hierarchy"];
  const exact = assembleColumns(rows(
    table,
    [
      [0, 5, 8], [1, 6, 1], [2, ABSENT, 2], [3, 3, 3], [4, 7, 9],
      [ABSENT, ABSENT, ABSENT], [ABSENT, ABSENT, ABSENT], [1, 0, 0],
    ],
    [
      [10, 12], [0, 1], [1, 2], [11, 13], [11, 13], [ABSENT, ABSENT], [0, 1], [0, 0],
    ],
    [[1, 0.5, 0.25], [0, 0, 0], [0.5, 0.5]],
  ));
  // The exact path's offsets are the UTF-8 running sums, not the code-unit counts that sized
  // the fast path's buffer: two é's are four bytes against two code units.
  const blobBytes = table.reduce((sum, value) => sum + Buffer.byteLength(value, "utf8"), 0);
  const declared = new DataView(exact.buffer).getUint32(20, true);
  assert.equal(declared, blobBytes, "the header declares the UTF-8 blob length");
  assert.notEqual(blobBytes, table.join("").length, "the two widths really do differ here");
  const handle = m.buildColumns(exact);
  assert.equal(m.nodeCount(handle), 3);
  const ids = idsOf(m, handle);
  assert.equal(ids[0], "n-é", "the multi-byte id came back as itself");
  assert.equal(ids[1], "NEGCTL");
  assert.equal(ids[2], "n-2");
  assert.ok(!ids.some((id) => id.includes("�")), "nothing became U+FFFD on the way through");
});

test("a lone surrogate in the table is refused, naming its index", () => {
  const broken = asciiRows();
  const table = [...broken.strings];
  table[0] = "n-\ud800";
  assert.throws(
    () => assembleColumns({ ...broken, strings: table }),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "strings[0]");
      assert.match(error.message, /not well-formed/);
      return true;
    },
  );
});

test("two lone surrogates that join into a valid pair are still refused", () => {
  const broken = asciiRows();
  // Joined, "\ud800" + "\udc00" is U+10000: one well-formed code point, so a check on the
  // joined text alone would pass and both entries would read back as U+FFFD. The check is
  // per string for exactly this reason.
  const table = [...broken.strings];
  table[0] = "\ud800";
  table[5] = "\udc00";
  assert.throws(
    () => assembleColumns({ ...broken, strings: table }),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "strings[0]");
      return true;
    },
  );
});

test("a column of the wrong length is refused, naming the array", () => {
  const built = asciiRows();
  const short = { ...built, nodeCells: new Uint32Array(built.nodeCells.length - 1) };
  assert.throws(
    () => assembleColumns(short),
    (error) => {
      assert.ok(error instanceof GraphMotorError, String(error));
      assert.match(error.message, /nodeCells/);
      return true;
    },
  );
  const mismatched = { ...built, versions: new Float64Array(built.versions.length + 1) };
  assert.throws(
    () => assembleColumns(mismatched),
    (error) => {
      assert.ok(error instanceof GraphMotorError, String(error));
      assert.match(error.message, /versions/);
      return true;
    },
  );
  assert.doesNotThrow(() => assembleColumns(built), "the lengths that do match still build");
});