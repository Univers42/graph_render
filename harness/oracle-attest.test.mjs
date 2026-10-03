// Unit tests for the seal the fixture-reading arms share (m41, M24) and for the layout
// arm's manifest guards (m72, m74).
//
//   scripts/orch/node-slim.sh node --test harness/oracle-attest.test.mjs

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { attest, readSeal, refuseChangedBytes, sealPathFor, sha256Hex, treeFingerprint } from "./oracle-attest.mjs";
import { nodeValue } from "./oracle-layout-value.mjs";

const DIGEST = sha256Hex("the measured bytes");
const OTHER = sha256Hex("different bytes");
const FINGERPRINT = "f".repeat(64);
const seal = (path, over = {}) => ({ sealPath: path, gate: "test", fingerprint: FINGERPRINT, sha256: DIGEST, pass: true, ...over });

test("attest records the first sighting and reports it", () => {
  const path = sealPathFor(mkdtempSync(join(tmpdir(), "gm-seal-")), "test");
  const seen = attest(seal(path));
  assert.equal(seen.attested, false);
  assert.equal(seen.sha256, DIGEST);
  assert.deepEqual(readSeal(path), { gate: "test", fingerprint: FINGERPRINT, scope: "", sha256: DIGEST, pass: true });
});

test("refuseChangedBytes accepts the same bytes a passing run recorded", () => {
  const path = sealPathFor(mkdtempSync(join(tmpdir(), "gm-seal-")), "test");
  attest(seal(path));
  assert.equal(refuseChangedBytes(seal(path)).sha256, DIGEST);
});

test("refuseChangedBytes refuses edited bytes under an unchanged tree (m41, M24)", () => {
  const path = sealPathFor(mkdtempSync(join(tmpdir(), "gm-seal-")), "test");
  attest(seal(path));
  assert.throws(() => refuseChangedBytes(seal(path, { sha256: OTHER })), /changed under an unchanged tree .* -> /);
});

test("a tree edit starts a new epoch rather than refusing", () => {
  const path = sealPathFor(mkdtempSync(join(tmpdir(), "gm-seal-")), "test");
  attest(seal(path));
  const next = attest(seal(path, { fingerprint: "a".repeat(64), sha256: OTHER }));
  assert.equal(next.reattested, true);
  assert.equal(next.sha256, OTHER);
});

test("a different fixture set on the same tree is a new scope, not a tamper", () => {
  const path = sealPathFor(mkdtempSync(join(tmpdir(), "gm-seal-")), "test");
  attest(seal(path, { scope: "seeds=8" }));
  assert.equal(attest(seal(path, { scope: "seeds=1000", sha256: OTHER })).sha256, OTHER);
  assert.throws(() => refuseChangedBytes(seal(path, { scope: "seeds=1000" })), /changed under an unchanged tree/);
});

test("a failing run's seal is not a baseline to defend", () => {
  const path = sealPathFor(mkdtempSync(join(tmpdir(), "gm-seal-")), "test");
  attest(seal(path, { pass: false }));
  assert.equal(refuseChangedBytes(seal(path, { sha256: OTHER })).sha256, OTHER);
});

test("treeFingerprint moves with the bytes it covers and ignores what it does not", () => {
  const root = mkdtempSync(join(tmpdir(), "gm-tree-"));
  mkdirSync(join(root, "a"), { recursive: true });
  mkdirSync(join(root, "b"), { recursive: true });
  writeFileSync(join(root, "a", "one.txt"), "one\n");
  writeFileSync(join(root, "b", "two.txt"), "two\n");
  const before = treeFingerprint(root, ["a"]);
  assert.equal(before.length, 64);
  assert.notEqual(before, treeFingerprint(root, ["a", "b"]));
  writeFileSync(join(root, "b", "two.txt"), "two changed\n");
  assert.equal(before, treeFingerprint(root, ["a"]), "an entry outside the set does not move the fingerprint");
  writeFileSync(join(root, "a", "one.txt"), "one changed\n");
  assert.notEqual(before, treeFingerprint(root, ["a"]));
});

test("treeFingerprint is order-independent over the entries", () => {
  const root = mkdtempSync(join(tmpdir(), "gm-tree-"));
  mkdirSync(join(root, "a"), { recursive: true });
  mkdirSync(join(root, "b"), { recursive: true });
  writeFileSync(join(root, "a", "one.txt"), "one\n");
  writeFileSync(join(root, "b", "two.txt"), "two\n");
  assert.equal(treeFingerprint(root, ["a", "b"]), treeFingerprint(root, ["b", "a"]));
});

test("nodeValue gives the virtual root 0 and a NaN-weight real node the epsilon (m72)", () => {
  assert.equal(nodeValue({ id: null, weight: null }), 0);
  assert.equal(nodeValue({ id: null, weight: 5 }), 0, "the root is discriminated by its id, not its weight");
  assert.equal(nodeValue({ id: 3, weight: null }), 1e-6, "a real node whose f32 weight serialised as null is not the root");
  assert.equal(nodeValue({ id: 3, weight: Number.NaN }), 1e-6);
  assert.equal(nodeValue({ id: 3, weight: -1 }), 1e-6);
  assert.equal(nodeValue({ id: 3, weight: 0 }), 1e-6);
  assert.equal(nodeValue({ id: 3, weight: 2.5 }), 2.5);
});

test("a fixture tree with a null-weight real node is measured, not summed as the root", () => {
  const line = { seed: 1, tree: { id: null, weight: null, children: [{ id: 0, weight: null }, { id: 1, weight: 0.5 }] } };
  const values = [line.tree, ...line.tree.children].map(nodeValue);
  assert.deepEqual(values, [0, 1e-6, 0.5], "the NaN-weight node contributes WEIGHT_EPSILON, not 0");
  assert.ok(values[1] > 0, "a real node never contributes 0");
});

test("the emitted fixtures the layout arm reads carry the shapes the guards assume", () => {
  const path = join(import.meta.dirname, "..", "target", "oracle-fixtures", "layouts.jsonl");
  let text;
  try {
    text = readFileSync(path, "utf8");
  } catch {
    return; // emit-fixtures has not run on this host; the guards are covered above.
  }
  const first = JSON.parse(text.split("\n", 1)[0]);
  assert.equal(first.seed, 0);
  assert.equal(first.tree.id, null, "the virtual root is discriminated by id === null");
  assert.equal(first.tree.weight, null);
  assert.ok(first.tree.children.every((child) => typeof child.id === "number"));
  assert.equal(first.tree.children.length, first.tidy.x.length, "tidy.x holds one entry per real node");
});