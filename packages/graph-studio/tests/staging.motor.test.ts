// The staging path's two branches over the real motor: `gm_alloc` the document, then
// `encodeInto` it in place when every character is one ASCII byte, or fall back to the
// encoded array when one is not. The digests below are the snapshot bytes the pre-change
// staging path produced for these two documents, so a byte lost or doubled anywhere in
// either branch moves a digest. Each test is the other's negative control: the wide
// document is the same graph with one id's characters made multi-byte, so a fallback that
// stopped at the character count (or a fast path that trusted a byte count as a character
// count) shows up as one digest matching and the other not.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";

import { createMotor, type Motor } from "../../../crates/graph-sdk-js/src/index.ts";
import { edge, node } from "./support.ts";
import { SKIP, WASM } from "./motor.ts";

const WIDE_ID = "n-1 é中😀";
const ASCII_DOC = JSON.stringify({
  version: 1,
  nodes: [
    node("n-0", { kind: "database", database_id: "db-0", label: "Graph notes", group: "Layout", weight: 1 }),
    node("n-1", { kind: "note", label: "Layout design", group: "Layout", weight: 0.5, has_note: true }),
    node("n-2", { label: "Agent draft", group: "Memory", weight: 0.25 }),
  ],
  edges: [
    { ...edge("e-0", "n-1", "n-0"), kind: "hierarchy", directed: true, child_first: false },
    { ...edge("e-1", "n-2", "n-1"), kind: "note_link", child_first: false },
  ],
});
const WIDE_DOC = ASCII_DOC.replaceAll("n-1", WIDE_ID);
const ASCII_DIGEST = "0b8f394e1a9513f188c5b1b93ec4d0a2c8bbcfa8054cd154e7e0844b17ec69a9";
const WIDE_DIGEST = "b5dedb28784f24928dd70f6cd6e4163bc7e28e0fa2337ad60ff874e232304dfb";

/** Builds `text`, runs the layout the perf driver uses, and answers with the graph it made. */
async function built(text: string): Promise<{ readonly digest: string; readonly json: string; readonly length: number }> {
  const motor: Motor = await createMotor(WASM ?? new Uint8Array(0));
  const handle = motor.build(text);
  assert.equal(motor.nodeCount(handle), 3, "the fixture is three nodes");
  motor.layout(handle, "layout.random");
  const bytes = motor.toBytes(handle);
  const json = motor.toJSON(handle);
  motor.release(handle);
  return { digest: createHash("sha256").update(bytes).digest("hex"), json, length: bytes.length };
}

test("an ASCII document is staged in place and builds the graph the copy path built", { skip: SKIP }, async () => {
  assert.equal(ASCII_DOC.length, new TextEncoder().encode(ASCII_DOC).length, "the fixture must be ASCII");
  const ascii = await built(ASCII_DOC);
  assert.deepEqual([ascii.digest, ascii.length], [ASCII_DIGEST, 120]);
  assert.ok(ascii.json.includes('"n-1"'));
});

test("a document with a multi-byte id falls back and builds the graph the copy path built", { skip: SKIP }, async () => {
  assert.notEqual(WIDE_DOC.length, new TextEncoder().encode(WIDE_DOC).length, "the fixture must need the fallback");
  const wide = await built(WIDE_DOC);
  assert.deepEqual([wide.digest, wide.length], [WIDE_DIGEST, 128]);
  assert.ok(wide.json.includes(WIDE_ID), "the wide id survives as written, not as a replacement character");
});