// The TypeScript arm of the oracle differential (prompts/phase-01-topology.md step 9).
//
//   node --experimental-strip-types --experimental-loader ./tests/ts-extension-loader.mjs \
//        harness/oracle-diff.mjs [fixtures-dir]        (npm run oracle:diff)
//
// Reads what `graph-cli emit-fixtures` wrote (target/oracle-fixtures by default), runs
// every case through the oracle in src/, and compares its canonical JSON byte for byte
// with graph-core's line in expect.jsonl. A line may differ only where a decided
// deviation says it must, and only in the exact way that deviation predicts:
//   H1  makeEdgeId, undirected: the oracle orders endpoints by localeCompare, graph-core
//       by UTF-8 bytes (docs/decisions/h1-byte-order.md);
//   H9  layoutGroups: the oracle stores groups in a Uint8Array, graph-core in u32
//       (docs/decisions/h9-group-width.md; the transcription is in oracle-h9.mjs).
// Anything else is an unexplained mismatch and fails the run.
//
// Exit codes follow graph-cli: 0 pass · 1 ran and failed · 2 could not run. The verdict
// is recorded in <gates>/oracle-diff.json for the capabilities ledger.

import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import { attest, sealPathFor } from "./oracle-attest.mjs";
import { evaluator } from "./oracle-diff-eval.mjs";
import { checkBinaryContract } from "./oracle-wire-bytes.mjs";
import { checkTranscription, groupModel, h9Explains, widenedGroups } from "./oracle-h9.mjs";

const ROOT = resolve(import.meta.dirname, "..");
const FIXTURES = resolve(process.argv[2] ?? join(ROOT, "target", "oracle-fixtures"));
const GATES = process.env.GM_GATES_DIR ?? join(ROOT, "target", "gates");
const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const read = (path) => readFileSync(path, "utf8");

function fail(message) {
  process.stderr.write(`oracle-diff: could not run: ${message}\n`);
  process.exit(2);
}

// The 17 public functions (prompt.md §7.4), each with the module src/index.ts exports it
// from; hashString (H4) is internal to src/core/math.ts and imported from there.
const PUBLIC = {
  "./core/model/model": ["indexModel", "emptyModel", "nodesEqual"],
  "./core/model/ids": ["makeRecordNodeId", "makeNoteNodeId", "makeTagNodeId", "makeEdgeId", "parseNodeId"],
  "./core/model/weights": ["applyDegreeWeights"],
  "./core/model/edgeKind": ["edgeKindFromType"],
  "./core/model/diff": ["diffGraph", "isEmptyPatch", "edgesEqual"],
  "./core/model/legend": ["deriveLegend"],
  "./core/model/neighborhood": ["neighborhood", "neighborhoodEdges"],
  "./core/model/synthetic": ["buildSyntheticModel"],
};

const load = (path) => import(pathToFileURL(join(ROOT, "src", path)).href);

/**
 * The functions under test, each checked to be the one src/index.ts publishes, from the
 * same module. src/index.ts itself is not imported: it pulls in React and the canvas.
 */
async function publicSurface() {
  const exported = new Map();
  const blocks = read(join(ROOT, "src", "index.ts")).matchAll(/export\s*\{([^}]*)\}\s*from\s*"([^"]+)"/g);
  for (const [, names, from] of blocks) {
    for (const name of names.split(",").map((n) => n.trim())) exported.set(name, from);
  }
  const oracle = {};
  for (const [module, names] of Object.entries(PUBLIC)) {
    const loaded = await load(`${module.slice(2)}.ts`);
    for (const name of names) {
      if (exported.get(name) !== module) fail(`src/index.ts does not export ${name} from ${module}`);
      if (typeof loaded[name] !== "function") fail(`${module} has no function ${name}`);
      oracle[name] = loaded[name];
    }
  }
  oracle.hashString = (await load("core/math.ts")).hashString;
  return oracle;
}

/** The tree fingerprint, computed exactly as graph-cli's evidence.rs computes it. */
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
    manifest = JSON.parse(read(join(FIXTURES, "manifest.json")));
  } catch (error) {
    fail(`${FIXTURES}/manifest.json: ${error.message} (run graph-cli emit-fixtures)`);
  }
  for (const [file, digest] of Object.entries(manifest.sha256)) {
    if (sha256(readFileSync(join(FIXTURES, file))) !== digest) fail(`${file} does not match its manifest digest`);
  }
  if (sha256(readFileSync(join(ROOT, manifest.adversarial.path))) !== manifest.adversarial.sha256) {
    fail(`${manifest.adversarial.path} changed since the fixtures were emitted`);
  }
  if (fingerprint(manifest.fingerprinted) !== manifest.fingerprint) {
    fail("the tree changed since the fixtures were emitted: re-run graph-cli emit-fixtures");
  }
  const lines = (file) => read(join(FIXTURES, file)).split("\n").filter((l) => l !== "");
  const cases = lines("cases.jsonl");
  const expect = lines("expect.jsonl");
  if (cases.length !== expect.length) fail(`${cases.length} cases but ${expect.length} expected lines`);
  const pairs = JSON.parse(read(join(ROOT, manifest.adversarial.path))).pairs;
  // The two files' own bytes, so the seal attests what was measured and not only what the
  // manifest says about it. The manifest's digests are written by whoever wrote the
  // fixtures, so a hand-edited line plus a re-sealed digest passes every guard above; the
  // seal is what catches that, and `writeRecord` consults it only on a passing verdict — a
  // run that found a mismatch is already red, and refusing there would hide the mismatch
  // that is the real news.
  const digest = sha256(`${sha256(read(join(FIXTURES, "cases.jsonl")))}\0${sha256(read(join(FIXTURES, "expect.jsonl")))}`);
  return { manifest, cases, expect, pairs, digest };
}

const utf8First = (s, t) => Buffer.compare(Buffer.from(s), Buffer.from(t)) <= 0;

/** H1: the oracle id is the localeCompare order, graph-core's the byte order, and the two orders differ. */
function h1Explains(a, got, want) {
  if (a.directed) return false;
  const tail = `:${a.kind}:${a.label}`;
  const ordered = (first) => (first ? `${a.source}--${a.target}` : `${a.target}--${a.source}`) + tail;
  const byBytes = utf8First(a.source, a.target);
  const byLocale = a.source.localeCompare(a.target) <= 0;
  return byBytes !== byLocale && JSON.parse(want) === ordered(byBytes) && JSON.parse(got) === ordered(byLocale);
}

/**
 * The widest group index in an expected line, by iteration: the array is fixture-supplied
 * and `Math.max(...array)` is a spread into the call, so a wide case overflows the stack
 * and the arm reports "could not run" instead of a mismatch.
 */
function widestGroup(text) {
  const groups = JSON.parse(text);
  let widest = 0;
  for (const g of groups) if (typeof g === "number" && g > widest) widest = g;
  return widest;
}

/**
 * `widen(args)`: the untruncated groups of a layoutGroups case, for the H9 rule. Each
 * function's first unexplained mismatch is kept as its example, so the persisted record
 * names the line, the seed, the arguments and both sides without stdout.
 */
function compare({ cases, expect }, run, widen) {
  const functions = {};
  const mismatches = [];
  const h1Pairs = new Set();
  let h9Crossed = 0;
  for (let i = 0; i < cases.length; i += 1) {
    const { seed, fn, args } = JSON.parse(cases[i]);
    const got = run(fn, args);
    if (fn === "graph") continue;
    const counts = (functions[fn] ??= { cases: 0, equal: 0, declared: 0, unexplained: 0 });
    counts.cases += 1;
    if (fn === "layoutGroups" && widestGroup(expect[i]) >= 256) h9Crossed += 1;
    if (got === expect[i]) counts.equal += 1;
    else if (fn === "makeEdgeId" && h1Explains(args, got, expect[i])) {
      counts.declared += 1;
      h1Pairs.add(JSON.stringify([args.source, args.target]));
    } else if (fn === "layoutGroups" && h9Explains(widen(args), got, expect[i])) counts.declared += 1;
    else {
      counts.unexplained += 1;
      counts.example ??= {
        line: i + 1,
        seed,
        args: clip(JSON.stringify(args)),
        oracle: clip(got),
        core: clip(expect[i]),
      };
      mismatches.push({ line: i + 1, seed, fn, args: clip(JSON.stringify(args)), oracle: clip(got), core: clip(expect[i]) });
    }
  }
  return { functions, mismatches, h1Pairs, h9Crossed };
}

/** Every adversarial pair's `diverges` must be what this run observed for it. */
function checkDeclarations(pairs, h1Pairs) {
  return pairs
    .filter(({ a, b, diverges }) => (h1Pairs.has(JSON.stringify([a, b])) || h1Pairs.has(JSON.stringify([b, a]))) !== diverges)
    .map(({ a, b, diverges }) => `adversarial pair ${JSON.stringify([a, b])} declares diverges=${diverges}, observed ${!diverges}`);
}

const clip = (text) => (text.length > 300 ? `${text.slice(0, 300)}…` : text);

/** What the run must have covered, beyond matching: H1 and H9 seen, every case run. */
function coverageProblems(manifest, result) {
  const problems = [];
  const declaredH1 = result.functions.makeEdgeId?.declared ?? 0;
  if (declaredH1 < 3) problems.push(`H1: ${declaredH1} divergences observed, the fixture guarantees at least 3`);
  if (manifest.seeds >= 1000 && result.h9Crossed === 0) problems.push("H9: no layoutGroups case crossed 255 groups");
  for (const [fn, n] of Object.entries(manifest.counts)) {
    if (result.functions[fn]?.cases !== n) problems.push(`${fn}: manifest counts ${n} cases, ran ${result.functions[fn]?.cases ?? 0}`);
  }
  return problems;
}

function printReport({ manifest, cases }, result, problems) {
  console.log(`oracle-diff: ${manifest.seeds} seeds, ${cases.length} lines, node ${process.version}, icu ${process.versions.icu}`);
  for (const [fn, c] of Object.entries(result.functions).sort()) {
    console.log(`  ${fn.padEnd(20)} ${String(c.cases).padStart(6)} cases  ${String(c.equal).padStart(6)} equal  ${String(c.declared).padStart(5)} declared  ${c.unexplained} unexplained`);
  }
  console.log(`  H1 pairs observed diverging: ${result.h1Pairs.size} · H9 cases crossing 255 groups: ${result.h9Crossed}`);
  for (const m of result.mismatches.slice(0, 10)) {
    console.log(`  MISMATCH line ${m.line} seed ${m.seed} ${m.fn} ${m.args}\n    oracle ${m.oracle}\n    core   ${m.core}`);
  }
  for (const p of problems) console.log(`  PROBLEM ${p}`);
}

/**
 * Records the verdict, unless the tree moved while the run was reading it.
 *
 * The fixture bytes are sealed beside the record, and that is where a hand-edited
 * `expect.jsonl` with a re-sealed manifest digest is caught: this run would otherwise
 * compare against fixtures `emit-fixtures` never wrote and report a clean pass. A failing
 * run writes no seal — it is already red, and a refusal would replace its mismatch.
 */
function writeRecord({ manifest, digest }, result, pass) {
  if (fingerprint(manifest.fingerprinted) !== manifest.fingerprint) fail("the tree changed during the run: not recorded");
  const seal = pass
    ? attest({ sealPath: sealPathFor(GATES, "oracle-diff"), gate: "oracle-diff fixtures", fingerprint: manifest.fingerprint, sha256: digest, pass })
    : { sealed: false };
  const record = {
    gate: "oracle-diff",
    fingerprint: manifest.fingerprint,
    seeds: manifest.seeds,
    pass,
    runtime: { node: process.version, icu: process.versions.icu, locale: Intl.DateTimeFormat().resolvedOptions().locale },
    fixtures: { sha256: digest, seal },
    functions: result.functions,
    h1: { pairs: result.h1Pairs.size },
    h9: { crossed: result.h9Crossed },
  };
  mkdirSync(GATES, { recursive: true });
  writeFileSync(join(GATES, "oracle-diff.json"), `${JSON.stringify(record, null, 2)}\n`);
}

async function main() {
  const fixtures = loadFixtures();
  checkTranscription(ROOT);
  const binary = checkBinaryContract(ROOT);
  const oracle = await publicSurface();
  const result = compare(fixtures, evaluator(oracle), (args) => widenedGroups(groupModel(oracle, args)));
  const problems = [...checkDeclarations(fixtures.pairs, result.h1Pairs), ...coverageProblems(fixtures.manifest, result)];
  const pass = result.mismatches.length === 0 && problems.length === 0;
  printReport(fixtures, result, problems);
  console.log(`  binary contract (${binary.snapshot}): magic/format/dim/padding, CSR framing and node column order as documented`);
  writeRecord(fixtures, result, pass);
  console.log(pass ? "PASS" : `FAIL: ${result.mismatches.length} unexplained mismatches, ${problems.length} problems`);
  process.exit(pass ? 0 : 1);
}

try {
  await main();
} catch (error) {
  fail(error.stack ?? String(error));
}
