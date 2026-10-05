// The wasm arm's refusals, pinned without a gate run.
//
// Every case here is a negative control for a review finding in
// `docs/reviews/review-harness-sdk.md`: m37/U4 (`hash 0` was a vacuous pass), m37/m38 (the
// C20 pair's two halves were keyed off two different names and a half-named pair was a
// silent skip), m39 (a module missing an export threw a raw `TypeError` at exit 1 instead
// of the header's exit 2), m40 (the file was 394 lines against the house's 300).
//
// Run: `scripts/orch/node-slim.sh node --test harness/wasm-run.test.mjs`
//
// The cases that need the real artifact skip themselves when it has not been built, so
// this file is runnable on a tree with no wasm toolchain; the artifact itself is what
// `scripts/orch/gr cargo build -p graph-wasm --target wasm32-unknown-unknown --release`
// makes.

import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, existsSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { C20_PAIR, c20Plan, parseSeedCount, Refusal, SHIM_LAYOUT } from "./wasm-run/lib.mjs";
import { missingExports, expectedExports } from "./wasm-run/module.mjs";
import { parseShard, seedStride, WHOLE_SHARD } from "./wasm-run/hash.mjs";

const HARNESS = dirname(fileURLToPath(import.meta.url));
const ARM = join(HARNESS, "wasm-run.mjs");
const ARTIFACT = join(HARNESS, "..", "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");
const built = existsSync(ARTIFACT);
const needsArtifact = { skip: built ? false : "graph_wasm.wasm has not been built" };

/// The arm as a child process: its status, stdout and stderr. `--experimental-strip-types`
/// is passed so one command line works for every mode, including the SDK-importing one.
function arm(...args) {
  const child = spawnSync(process.execPath, ["--experimental-strip-types", ARM, ...args], {
    encoding: "utf8",
  });
  return { status: child.status, stdout: child.stdout, stderr: child.stderr };
}

function refused(...args) {
  const run = arm(...args);
  assert.equal(run.status, 2, `expected exit 2 (could not run), got ${run.status}: ${run.stderr}`);
  assert.doesNotMatch(run.stderr, /TypeError| at Object| at Module/, `a stack, not a refusal:\n${run.stderr}`);
  return run.stderr;
}

// --- hg-shard: `--shard i/K` refuses what `hashgate/shard.rs` refuses ------------------------

test("a shard that cannot exist is refused by name, on the Rust side's table", () => {
  // The table of `a_shard_that_cannot_exist_is_refused_by_name` (`hashgate/shard/tests.rs`).
  for (const text of ["1/0", "0/0", "1/1", "7/2", "x/2", "2/x", "2", "2/", "", "1/2/3", "-1/2"]) {
    const named = (err) => err instanceof Refusal && err.message.includes(`bad shard ${JSON.stringify(text)}`);
    assert.throws(() => parseShard(text), named, `${text} should be refused by name`);
  }
  assert.throws(() => parseShard("0/0"), /K is 0/);
  assert.throws(() => parseShard("7/2"), /i is 7, so it is not one of the 2 shards/);
  assert.deepEqual(parseShard("0/1"), WHOLE_SHARD);
  assert.deepEqual(parseShard("2/3"), { index: 2, count: 3 });
});

test("the shards of a run stride its seeds and cover each one once", () => {
  assert.deepEqual([...seedStride({ index: 2, count: 3 }, 11)], [2, 5, 8]);
  const seen = [0, 1, 2].flatMap((index) => [...seedStride({ index, count: 3 }, 7)]);
  assert.deepEqual(seen.sort((a, b) => a - b), [0, 1, 2, 3, 4, 5, 6]);
});

// --- U4 / m37: `hash 0` was a vacuous pass -------------------------------------------------

test("a zero seed count is refused instead of passing vacuously", () => {
  assert.throws(() => parseSeedCount("0"), Refusal);
  assert.throws(() => parseSeedCount("0"), /0 seeds: a tally over nothing proves nothing/);
  // The neighbours the arm does accept, so the refusal is about zero and not about parsing.
  assert.equal(parseSeedCount("1"), 1);
  assert.equal(parseSeedCount("4294967295"), 4294967295);
});

test("a seed count that is not a non-negative integer is refused", () => {
  for (const text of ["", "-1", "1.5", "x", "0x8", "1e3", " 1", undefined]) {
    assert.throws(() => parseSeedCount(text), Refusal, `${text} should be refused`);
  }
  assert.throws(() => parseSeedCount("4294967296"), /bad seed count/);
});

test("`hash 0` writes no digest line and no C20 verdict, at exit 2", { ...needsArtifact }, () => {
  const stderr = refused(ARTIFACT, "hash", "0", SHIM_LAYOUT, ...c20PairTail());
  assert.match(stderr, /0 seeds: a tally over nothing proves nothing/);
  assert.doesNotMatch(stderr, /C20 ok/, "a comparison over no seed is not a pass");
});

// --- m37 / m38: the C20 pair's two halves --------------------------------------------------

test("the C20 pair and the arm's stage table name one layout on both sides", async () => {
  assert.deepEqual([...C20_PAIR], [SHIM_LAYOUT, "transport.wasm.columnar"]);
  // `createAbi` touches nothing until a stage is hashed, so a stub `exports` is enough to
  // read the table's own key set. Every stage hashable without a registry is keyed off one
  // half of the pair, so renaming a half cannot leave the comparison's two sides apart.
  const { createAbi } = await import("./wasm-run/abi.mjs");
  const table = createAbi({}).stageBytes;
  assert.deepEqual(Object.keys(table), ["topology", SHIM_LAYOUT, "transport.wasm.columnar"]);
  assert.ok(C20_PAIR.every((stage) => stage in table));
  assert.equal(Object.getPrototypeOf(table), null, "a null prototype refuses `toString` by name");
});

test("c20Plan reads a whole, half or absent pair out of the requested stages", () => {
  assert.equal(c20Plan(["topology", ...C20_PAIR]).state, "pair");
  assert.equal(c20Plan(["topology"]).state, "neither");
  const half = c20Plan([C20_PAIR[1]]);
  assert.equal(half.state, "half");
  assert.deepEqual(half.present, ["transport.wasm.columnar"]);
  assert.deepEqual(half.missing, ["layout.grid"]);
});

test("a multi-stage run naming one half of the C20 pair refuses, by name", { ...needsArtifact }, () => {
  const stderr = refused(ARTIFACT, "hash", "1", "topology", "transport.wasm.columnar");
  assert.match(stderr, /C20 pair is half of that/);
  assert.match(stderr, /transport\.wasm\.columnar without layout\.grid/);
  assert.match(stderr, /needs both layout\.grid and transport\.wasm\.columnar in one invocation/);
  assert.doesNotMatch(stderr, /C20 ok/, "a half-named pair is not a pass");
});

test("a single-stage probe of one half says on stderr that nothing was proved", { ...needsArtifact }, () => {
  const run = arm(ARTIFACT, "hash", "1", "transport.wasm.columnar");
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stdout, /^transport\.wasm\.columnar 0 [0-9a-f]{64}$/m);
  assert.match(run.stderr, /C20 NOT checked/);
  assert.doesNotMatch(run.stderr, /C20 ok/, "one half alone proves nothing");
});

test("the whole pair in one invocation still prints the C20 verdict", { ...needsArtifact }, () => {
  const run = arm(ARTIFACT, "hash", "1", SHIM_LAYOUT, ...c20PairTail());
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stderr, /C20 ok — transport\.wasm\.columnar == layout\.grid on all 1 seeds/);
});

// --- m39: a module missing an export refused by name ----------------------------------------

test("a module without the exports this mode drives is refused by name", async () => {
  const dir = mkdtempSync(join(tmpdir(), "wasm-run-truncated-"));
  const path = join(dir, "truncated.wasm");
  writeFileSync(path, truncatedModule());
  assert.ok(WebAssembly.Module.exports(await WebAssembly.compile(readFileSync(path))).length > 0);

  const stderr = refused(path, "hash", "1", "topology");
  assert.match(stderr, /missing \d+ export\(s\)/);
  assert.match(stderr, /gm_snapshot_bytes/, "the missing export is named, not a TypeError");
  assert.match(stderr, /gm_analysis_count/);
  assert.match(stderr, /not a graph_wasm build/);
  // The one export the truncated module does carry is credited, not refused: the check is
  // about what is absent, not about the module being small.
  assert.doesNotMatch(stderr, /gm_seed_ingest/);
});

test("missingExports keys on the wasm export kind, not just the name", () => {
  const expected = expectedExports("hash");
  assert.deepEqual(missingExports([], expected).map(([name]) => name), expected.map(([n]) => n));
  // A `memory` exported under the wrong kind is as unusable as an absent one, and the
  // check has to catch it here rather than one `DataView` later. `missingExports` reads
  // `WebAssembly.Module.exports`' own `{name, kind}` records.
  const flipped = expected.map(([name, kind]) => ({ name, kind: kind === "memory" ? "function" : kind }));
  assert.deepEqual(missingExports(flipped, expected).map(([name]) => name), ["memory"]);
  // probe drives one optional export, so it is charged for nothing but `memory`.
  assert.deepEqual(expectedExports("probe"), [["memory", "memory"]]);
  assert.deepEqual(expectedExports("--assert-zero-copy"), [["memory", "memory"]]);
});

test("a module that imports anything is still refused before the export check", async () => {
  const dir = mkdtempSync(join(tmpdir(), "wasm-run-importing-"));
  const path = join(dir, "importing.wasm");
  writeFileSync(path, importingModule());
  assert.match(refused(path, "hash", "1", "topology"), /module imports host/);
});

// --- m40: the house's file-size limit --------------------------------------------------------

test("every file of the wasm arm is within the house's 300-line limit", () => {
  const files = [ARM, ...readdirSync(join(HARNESS, "wasm-run")).map((name) => join(HARNESS, "wasm-run", name))];
  const sizes = files
    .filter((file) => file.endsWith(".mjs") && !file.endsWith(".test.mjs"))
    .map((file) => [file.slice(HARNESS.length + 1), readFileSync(file, "utf8").split("\n").length]);
  assert.ok(sizes.length >= 6, `expected the split to be real, found ${JSON.stringify(sizes)}`);
  for (const [name, lines] of sizes) {
    assert.ok(lines <= 300, `${name} is ${lines} lines, over the house's 300`);
  }
});

// --- helpers ---------------------------------------------------------------------------------

function c20PairTail() {
  return [C20_PAIR[1]];
}

/// A minimal but *valid* wasm module: one memory and one exported function, and nothing
/// else. Enough to compile, instantiate and import nothing — so the arm reaches its export
/// check and refuses there rather than on a trap. Every size below is LEB128, and the
/// section list is (type, function, memory, export); nothing is a stub.
function truncatedModule() {
  return Buffer.from([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    ...section(1, [1, 0x60, 0, 1, 0x7f]),
    ...section(3, [1, 0]),
    ...section(5, [1, 0x00, 1]),
    ...section(7, [2, ...exported("memory", 0x02, 0), ...exported("gm_seed_ingest", 0x00, 0)]),
    ...section(10, [1, 4, 0x00, 0x41, 0x00, 0x0b]),
  ]);
}

/// The same module with one import, so the import refusal is reachable without the real
/// graph_wasm (which imports nothing, by design).
function importingModule() {
  return Buffer.from([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00,
    ...section(1, [1, 0x60, 0, 1, 0x7f]),
    ...section(2, [1, ...utf8("env"), ...utf8("host"), 0x00, 0]),
    ...section(3, [1, 0]),
    ...section(5, [1, 0x00, 1]),
    ...section(7, [1, ...exported("memory", 0x02, 0)]),
    ...section(10, [1, 4, 0x00, 0x41, 0x00, 0x0b]),
  ]);
}

function exported(name, kind, index) {
  return [...utf8(name), kind, ...leb(index)];
}

function utf8(text) {
  const bytes = [...Buffer.from(text, "utf8")];
  return [...leb(bytes.length), ...bytes];
}

function section(id, body) {
  return [id, ...leb(body.length), ...body];
}

function leb(value) {
  const out = [];
  for (let rest = value; ; rest >>>= 7) {
    let byte = rest & 0x7f;
    if (rest >>>= 7) byte |= 0x80;
    out.push(byte);
    if (!rest) return out;
  }
}
