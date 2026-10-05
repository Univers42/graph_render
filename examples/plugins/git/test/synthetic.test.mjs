import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { parseLog } from "../map.mjs";

const SYNTHETIC = fileURLToPath(new URL("../synthetic.mjs", import.meta.url));

function generate(n, seed) {
  const dir = mkdtempSync(join(tmpdir(), "synthetic-"));
  const out = join(dir, "log");
  execFileSync(process.execPath, ["--experimental-strip-types", SYNTHETIC, `${n}`, `${seed}`, out]);
  return readFileSync(out, "utf8");
}

test("n = 1000, seed 1: parseLog takes every commit and keeps a two-parent merge", () => {
  const commits = parseLog(generate(1000, 1));
  assert.equal(commits.length, 1000);
  assert.equal(commits.filter((c) => c.parents.length === 2).length >= 1, true);
  assert.equal(commits.filter((c) => c.parents.length === 0).length, 1, "exactly one root");
});

test("the same seed writes the same log and a different seed does not", () => {
  assert.equal(generate(200, 7), generate(200, 7));
  assert.notEqual(generate(200, 7), generate(200, 8));
});

test("every commit is children-first: no parent appears after its child", () => {
  const commits = parseLog(generate(1000, 1));
  const seen = new Set();
  for (const commit of commits) {
    for (const parent of commit.parents) assert.ok(seen.has(parent), `${commit.hash} precedes its parent`);
    seen.add(commit.hash);
  }
});