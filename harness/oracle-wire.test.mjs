// Unit tests for the fixture wire helpers: canonical's refusals (m76), the binary-contract
// byte checks (m75), the CSR/header decoders' refusals, and the H9 transcription's anchor
// (M25) plus its single transcription (m61).
//
//   scripts/orch/node-slim.sh node --test harness/oracle-wire.test.mjs

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import test from "node:test";
import { canonical } from "./oracle-wire.mjs";
import { checkBinaryContract } from "./oracle-wire-bytes.mjs";
import { checkTranscription, h9Explains, layoutGroups, widenedGroups } from "./oracle-h9.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const BRIDGE = join("src", "core", "layout", "layoutBridge.ts");

/** The real layoutBridge.ts, in a temp root, optionally with `mutate` applied first. */
function tempTree(mutate) {
  const root = mkdtempSync(join(tmpdir(), "gm-h9-"));
  const path = join(root, BRIDGE);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, mutate ? mutate(readFileSync(join(ROOT, BRIDGE), "utf8")) : readFileSync(join(ROOT, BRIDGE), "utf8"));
  return root;
}

/** The twelve H9 lines, wherever they are in `text`. */
function block(text) {
  const lines = text.split("\n").map((line) => line.trim());
  const at = lines.findIndex((line) => line.startsWith("const nodeGroups = new Uint8Array(this.idList.length);"));
  return lines.slice(at, at + 12);
}

test("canonical refuses a non-finite number instead of writing it as null", () => {
  assert.equal(canonical({ x: null }), '{"x":null}');
  assert.throws(() => canonical({ x: Number.NaN }), /not finite/);
  assert.throws(() => canonical({ x: Number.POSITIVE_INFINITY }), /not finite/);
  assert.throws(() => canonical({ x: [1, Number.NEGATIVE_INFINITY] }), /not finite/);
});

test("canonical refuses an undefined member instead of dropping it", () => {
  assert.equal(canonical({ a: 1, b: 2 }), '{"a":1,"b":2}');
  assert.throws(() => canonical({ x: undefined }), /not finite|undefined/);
  assert.throws(() => canonical([1, undefined]), /not finite|undefined/);
  assert.throws(() => canonical(undefined), /undefined/);
});

test("canonical still sorts keys and round-trips nested values", () => {
  assert.equal(canonical({ b: [1, { d: 2, c: 3 }], a: "x" }), '{"a":"x","b":[1,{"c":3,"d":2}]}');
  assert.equal(canonical({ z: -0 }), '{"z":0}');
  assert.equal(canonical({ n: null }), '{"n":null}');
});

test("checkTranscription accepts the real layoutBridge.ts", () => {
  checkTranscription(ROOT);
});

test("checkTranscription refuses the block moved into a dead method (M25)", () => {
  const root = tempTree((text) => {
    const lines = text.split("\n");
    const at = lines.findIndex((l) => l.includes("const nodeGroups = new Uint8Array(this.idList.length);"));
    const moved = lines.slice(at, at + 12);
    const rest = [...lines.slice(0, at), ...lines.slice(at + 12)];
    const anchor = rest.findIndex((l) => l.includes("Push new physics params"));
    const dead = ["  private legacyGroups(model: GraphModel): Uint8Array {", ...moved.map((l) => (l ? `  ${l}` : l)), "    return nodeGroups;", "  }", ""];
    return [...rest.slice(0, anchor), ...dead, ...rest.slice(anchor)].join("\n");
  });
  assert.equal(block(readFileSync(join(root, BRIDGE), "utf8")).length, 12, "the negative case really moved the twelve lines");
  assert.throws(() => checkTranscription(root), /no longer holds the twelve lines/);
});

test("checkTranscription refuses a file with no rebuild method", () => {
  const root = tempTree((text) => text.replace("rebuild(model: GraphModel): void {", "rebuildLegacy(model: GraphModel): void {"));
  assert.throws(() => checkTranscription(root), /no rebuild method|never closed/);
});

test("layoutGroups and widenedGroups agree below 256 groups and differ above (m61)", () => {
  const model = (sources) => ({ nodes: sources.map((source, id) => ({ id, source })) });
  const few = model(["a", "b", "a", null, "b"]);
  assert.deepEqual(layoutGroups(few), [0, 1, 0, 0, 1]);
  assert.deepEqual(widenedGroups(few), layoutGroups(few));
  const many = model(Array.from({ length: 300 }, (_, i) => `s${i}`));
  assert.deepEqual(layoutGroups(many), many.nodes.map((_, i) => i & 0xff));
  assert.deepEqual(widenedGroups(many), many.nodes.map((_, i) => i));
});

test("h9Explains accepts only the mask/widen pair on the same groups", () => {
  const model = { nodes: [{ id: 0, source: "a" }, { id: 1, source: "b" }] };
  const wide = widenedGroups(model);
  assert.equal(h9Explains(wide, canonical(layoutGroups(model)), canonical(wide)), true);
  assert.equal(h9Explains(wide, canonical([9, 9]), canonical(wide)), false);
  assert.equal(h9Explains(wide, canonical(layoutGroups(model)), canonical([9, 9])), false);
});

test("checkBinaryContract decodes the contract's pinned 84-byte example (m75)", () => {
  const read = checkBinaryContract(ROOT);
  assert.equal(read.magic, "GMSN");
  assert.equal(read.format, "0.3");
  assert.equal(read.dim, 0);
  assert.equal(read.nodes, 2);
  assert.equal(read.edges, 1);
  assert.deepEqual(read.nodeIds, ["a", "bc"]);
  assert.deepEqual(read.edgeIds, ["e"]);
  assert.deepEqual(read.columns, ["x", "y"]);
  assert.deepEqual(read.source, [0]);
  assert.deepEqual(read.target, [1]);
  assert.deepEqual(read.x, [1, -2.5]);
  assert.deepEqual(read.y, [0, 0.5]);
  assert.equal(read.notes, 0);
  assert.equal(read.bytes, 84);
});

/** The contract, in a temp root, with `mutate` applied. */
function tempContract(mutate) {
  const root = mkdtempSync(join(tmpdir(), "gm-bytes-"));
  const dir = join(root, "docs", "contract");
  mkdirSync(dir, { recursive: true });
  const text = readFileSync(join(ROOT, "docs", "contract", "binary-layout.md"), "utf8");
  writeFileSync(join(dir, "binary-layout.md"), mutate(text));
  return root;
}

test("checkBinaryContract refuses a contract with no pinned example", () => {
  const root = tempContract((text) => text.replace("## The pinned 84-byte example", "## An example"));
  assert.throws(() => checkBinaryContract(root), /no '## The pinned 84-byte example' section/);
});

test("checkBinaryContract refuses a non-zero header padding byte (m75)", () => {
  const root = tempContract((text) => text.replace("| 15 | `00` | 1 | header padding | 0 |", "| 15 | `01` | 1 | header padding | 0 |"));
  assert.throws(() => checkBinaryContract(root), /padding byte 15 is 1/);
});

test("checkBinaryContract refuses a reserved dim byte (m75)", () => {
  const root = tempContract((text) => text.replace("| 14 | `00` | 1 | z channel | 0 |", "| 14 | `02` | 1 | z channel | 0 |"));
  assert.throws(() => checkBinaryContract(root), /dim byte 14 is 2/);
});

test("checkBinaryContract refuses a non-zero CSR pad byte (m75)", () => {
  const root = tempContract((text) => text.replace("| 43 | `00` | 1 | node.id padding | `padding(3) = 1` |", "| 43 | `01` | 1 | node.id padding | `padding(3) = 1` |"));
  assert.throws(() => checkBinaryContract(root), /node.id table padding byte 43 is 0x1/);
});

test("checkBinaryContract refuses decreasing CSR offsets (m75)", () => {
  const root = tempContract((text) => text.replace("| 32 | `01 00 00 00` | 4 | node.id offsets[1] | 1 |", "| 32 | `05 00 00 00` | 4 | node.id offsets[1] | 1 |"));
  assert.throws(() => checkBinaryContract(root), /node.id table offsets\[\d\] = \d+ decreases from \d+/);
});

test("checkBinaryContract refuses a contract whose pinned magic is not GMSN", () => {
  const root = tempContract((text) => text.replace("| 0 | `47 4D 53 4E` | 4 | magic | `\"GMSN\"` |", "| 0 | `47 4D 53 4F` | 4 | magic | `\"GMSO\"` |"));
  assert.throws(() => checkBinaryContract(root), /magic is "GMSO"/);
});

test("checkBinaryContract refuses a reordered node-column table (m75)", () => {
  const root = tempContract((text) => {
    const swapped = text.replace("| Point (`0`) | `x`, `y` | `x`, `y`, `z` | finite |", "| Point (`0`) | `y`, `x` | `y`, `x`, `z` | finite |");
    assert.notEqual(swapped, text, "the column table really changed");
    return swapped;
  });
  assert.throws(() => checkBinaryContract(root), /node columns decode to/);
});