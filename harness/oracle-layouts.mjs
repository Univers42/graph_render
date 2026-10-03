// The d3-hierarchy arm of the layout oracle differential
// (prompts/phase-03-deterministic-layouts.md's "the layout oracles" gate step).
//
//   node harness/oracle-layouts.mjs [fixtures-dir]        (graph-cli oracle-layouts)
//   node harness/oracle-layouts.mjs --dag [dump]          (harness/oracle-dag.mjs)
//
// Reads what `graph-cli emit-fixtures` wrote (layouts.jsonl / layout-manifest.json,
// target/oracle-fixtures by default). For every seed it rebuilds the exact tree
// tidy_tree.rs and treemap.rs's own module docs specify a future oracle must build, runs
// d3-hierarchy@3.1.2's `tree()` and `treemap().tile(treemapSquarify)` at their documented
// call sequences (both left at every other default), and compares every real node's
// tidy-tree `(x, y)` or treemap `(x, y, w, h)` against graph-core's own `f32` output.
//
// Every comparison rounds the oracle's f64 result through `Math.fround` and compares it
// exactly to graph-core's value (already an f32, parsed back as the identical f64): a
// match must be exact, never within a tolerance. In particular, a divergence that looks
// like a uniform scale or translation is still an unexplained failure here — the two
// ports either compute the same convention or they do not, and only a human reading the
// mismatches this harness prints can decide which; this file may not paper over one with
// slack.
//
// Exit codes follow graph-cli: 0 pass · 1 ran and failed · 2 could not run. The verdict is
// recorded in <gates>/oracle-layouts.json for the capabilities ledger. `crates/graph-cli/
// tests/cli.rs`'s `oracle_layouts_passes_on_emitted_fixtures_and_goes_red_on_a_wrong_line`
// is this harness's own negative control: it corrupts one emitted value (fixing the
// manifest digest up the way a real bug would leave it) and checks this file goes red.

import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, relative, resolve, sep } from "node:path";
import { hierarchy, tree, treemap, treemapSquarify } from "d3-hierarchy";
import { attest, refuseChangedBytes, sealPathFor } from "./oracle-attest.mjs";
import { nodeValue } from "./oracle-layout-value.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const FIXTURES = resolve(process.argv[2] ?? join(ROOT, "target", "oracle-fixtures"));
const GATES = process.env.GM_GATES_DIR ?? join(ROOT, "target", "gates");
const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const read = (path) => readFileSync(path, "utf8");

function fail(message) {
  process.stderr.write(`oracle-layouts: could not run: ${message}\n`);
  process.exit(2);
}

/**
 * The tree fingerprint, computed exactly as graph-cli's `evidence.rs` (`fingerprint_of`)
 * computes it. Restated here rather than imported from `oracle-diff.mjs`: `layouts.jsonl`
 * is its own manifest, generator and oracle, kept deliberately separate from the topology
 * oracle's shape (see `oracle_fixtures/layouts.rs`'s module doc), so this harness reads
 * nothing from that one.
 */
function fingerprint(entries) {
  const files = [];
  const walk = (path) => {
    if (statSync(path).isFile()) files.push(relative(ROOT, path).split(sep).join("/"));
    else for (const child of readdirSync(path)) walk(join(path, child));
  };
  for (const entry of entries) walk(join(ROOT, entry));
  files.sort((a, b) => Buffer.compare(Buffer.from(a), Buffer.from(b)));
  return sha256(files.map((f) => `${f}\0${sha256(readFileSync(join(ROOT, f)))}\n`).join(""));
}

function loadFixtures() {
  let manifest;
  try {
    manifest = JSON.parse(read(join(FIXTURES, "layout-manifest.json")));
  } catch (error) {
    fail(`${FIXTURES}/layout-manifest.json: ${error.message} (run graph-cli emit-fixtures)`);
  }
  for (const [file, digest] of Object.entries(manifest.sha256)) {
    if (sha256(readFileSync(join(FIXTURES, file))) !== digest) fail(`${file} does not match its manifest digest`);
  }
  if (fingerprint(manifest.fingerprinted) !== manifest.fingerprint) {
    fail("the tree changed since the fixtures were emitted: re-run graph-cli emit-fixtures");
  }
  // `seeds` is trusted from a manifest the emitter wrote, so `seeds: 0` with an empty
  // layouts.jsonl and a re-sealed digest satisfies the check below and measures nothing.
  if (!Number.isInteger(manifest.seeds) || manifest.seeds < 1) {
    fail(`layout-manifest.json declares seeds=${JSON.stringify(manifest.seeds)}: a differential needs at least one case`);
  }
  const lines = read(join(FIXTURES, "layouts.jsonl"))
    .split("\n")
    .filter((l) => l !== "")
    .map((l) => JSON.parse(l));
  if (lines.length !== manifest.seeds) {
    fail(`layout-manifest.json declares ${manifest.seeds} seeds, layouts.jsonl has ${lines.length}`);
  }
  // The manifest's digests are written by whoever wrote the fixtures, so a hand-edited line
  // plus a re-sealed digest passes every guard above. Refused here, before the comparison.
  const digest = digestOf(manifest);
  refuseChangedBytes({ sealPath: sealPathFor(GATES, "oracle-layouts"), gate: "oracle-layouts fixtures", fingerprint: manifest.fingerprint, sha256: digest });
  return { manifest, lines, digest };
}

/** Every real node (`data.id !== null`), indexed by its dense id, from anywhere in `node`'s subtree. */
function collectById(node, byId) {
  if (node.data.id !== null) byId[node.data.id] = node;
  for (const child of node.children ?? []) collectById(child, byId);
}

/** One function's tally: a case per real node compared, `equal`/`unexplained` of those. */
function tally(functions, id) {
  return (functions[id] ??= { cases: 0, equal: 0, declared: 0, unexplained: 0 });
}

/** Tidy tree: `d3.hierarchy(root, children)` then `d3.tree()(root)`, every default kept. */
function compareTidy(line, functions, mismatches) {
  const root = hierarchy(line.tree, (d) => d.children);
  tree()(root);
  const byId = new Array(line.tidy.x.length);
  collectById(root, byId);
  const counts = tally(functions, "layout.tree.tidy");
  for (let id = 0; id < byId.length; id += 1) {
    counts.cases += 1;
    const got = [Math.fround(byId[id].x), Math.fround(byId[id].y)];
    const want = [line.tidy.x[id], line.tidy.y[id]];
    if (got[0] === want[0] && got[1] === want[1]) counts.equal += 1;
    else {
      counts.unexplained += 1;
      mismatches.push({ seed: line.seed, fn: "layout.tree.tidy", id, oracle: got, core: want });
    }
  }
}

/** `x0,y0,x1,y1` recast the contract's centre/size way, each field cast to `f32` on its own. */
function boxOf(node) {
  return [
    Math.fround((node.x0 + node.x1) / 2),
    Math.fround((node.y0 + node.y1) / 2),
    Math.fround(node.x1 - node.x0),
    Math.fround(node.y1 - node.y0),
  ];
}

/** Squarified treemap: `hierarchy(...).sum(value).sort(descending)` then `treemap().tile(squarify)`. */
function compareTreemap(line, functions, mismatches) {
  const root = hierarchy(line.tree, (d) => d.children)
    .sum(nodeValue)
    .sort((a, b) => b.value - a.value);
  treemap().tile(treemapSquarify).size([1, 1])(root);
  const n = line.treemap.x.length;
  const byId = new Array(n);
  collectById(root, byId);
  const counts = tally(functions, "layout.treemap.squarified");
  for (let id = 0; id < n; id += 1) {
    counts.cases += 1;
    const got = boxOf(byId[id]);
    const want = [line.treemap.x[id], line.treemap.y[id], line.treemap.w[id], line.treemap.h[id]];
    if (got.every((v, i) => v === want[i])) counts.equal += 1;
    else {
      counts.unexplained += 1;
      mismatches.push({ seed: line.seed, fn: "layout.treemap.squarified", id, oracle: got, core: want });
    }
  }
}

function compare(lines) {
  const functions = {};
  const mismatches = [];
  for (const line of lines) {
    compareTidy(line, functions, mismatches);
    compareTreemap(line, functions, mismatches);
  }
  return { functions, mismatches };
}

function printReport(manifest, result) {
  console.log(`oracle-layouts: ${manifest.seeds} seeds, node ${process.version}`);
  for (const [fn, c] of Object.entries(result.functions).sort()) {
    console.log(`  ${fn.padEnd(28)} ${String(c.cases).padStart(7)} cases  ${String(c.equal).padStart(7)} equal  ${c.unexplained} unexplained`);
  }
  for (const m of result.mismatches.slice(0, 10)) {
    console.log(`  MISMATCH seed ${m.seed} ${m.fn} node ${m.id}\n    oracle ${JSON.stringify(m.oracle)}\n    core   ${JSON.stringify(m.core)}`);
  }
}

/**
 * The fixture bytes the run measured, over the files themselves rather than over the
 * manifest's claim about them: a hand-edited line with a re-sealed digest changes this.
 */
const digestOf = (manifest) => sha256(Object.keys(manifest.sha256).sort().map((f) => `${f}\0${sha256(readFileSync(join(FIXTURES, f)))}`).join("\n"));

/** Records the verdict, unless the tree moved while the run was reading it. */
function writeRecord({ manifest, digest }, result, pass) {
  if (fingerprint(manifest.fingerprinted) !== manifest.fingerprint) fail("the tree changed during the run: not recorded");
  const seal = attest({ sealPath: sealPathFor(GATES, "oracle-layouts"), gate: "oracle-layouts fixtures", fingerprint: manifest.fingerprint, sha256: digest, pass });
  const record = {
    gate: "oracle-layouts",
    fingerprint: manifest.fingerprint,
    seeds: manifest.seeds,
    pass,
    runtime: { node: process.version },
    fixtures: { sha256: digest, seal },
    functions: result.functions,
  };
  mkdirSync(GATES, { recursive: true });
  writeFileSync(join(GATES, "oracle-layouts.json"), `${JSON.stringify(record, null, 2)}\n`);
}

function main() {
  const fixtures = loadFixtures();
  const result = compare(fixtures.lines);
  const pass = result.mismatches.length === 0;
  printReport(fixtures.manifest, result);
  writeRecord(fixtures, result, pass);
  console.log(pass ? "PASS" : `FAIL: ${result.mismatches.length} unexplained mismatches`);
  process.exit(pass ? 0 : 1);
}

// `--dag` is the layered drawing's arm (dagre-d3-es crossing counts), kept in its own
// file because it shares nothing with the d3-hierarchy differential above but the name.
// The import is inside the try/catch like every other read this file makes: a missing or
// renamed dag harness is "could not run" (exit 2), not an unhandled rejection (exit 1).
try {
  if (process.argv[2] === "--dag") await import("./oracle-dag.mjs");
  else main();
} catch (error) {
  fail(error.stack ?? String(error));
}
