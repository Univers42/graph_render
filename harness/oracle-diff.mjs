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
//       (docs/decisions/h9-group-width.md).
// Anything else is an unexplained mismatch and fails the run.
//
// Exit codes follow graph-cli: 0 pass · 1 ran and failed · 2 could not run. The verdict
// is recorded in <gates>/oracle-diff.json for the capabilities ledger.

import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { join, relative, resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";
import { canonical, hex, unhex, node, edge, wireModel, wirePatch, wireLegend } from "./oracle-wire.mjs";

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

// H9: layoutGroups is not a function the oracle exports — it is the body of
// LayoutController's rebuild (src/core/layout/layoutBridge.ts:76-87), transcribed here.
// The transcription is refused unless every line of it is still in that file verbatim.
const H9_SOURCE = [
  "const nodeGroups = new Uint8Array(this.idList.length);",
  "const sourceIndex = new Map<string, number>();",
  "for (let i = 0; i < model.nodes.length; i += 1) {",
  "const source = model.nodes[i].source;",
  "if (source == null) continue;",
  "let g = sourceIndex.get(source);",
  "if (g === undefined) {",
  "g = sourceIndex.size;",
  "sourceIndex.set(source, g);",
  "nodeGroups[i] = g & 0xff;",
];

function layoutGroups(nodes) {
  const nodeGroups = new Uint8Array(nodes.length);
  const sourceIndex = new Map();
  for (let i = 0; i < nodes.length; i += 1) {
    const source = nodes[i].source;
    if (source == null) continue;
    let g = sourceIndex.get(source);
    if (g === undefined) {
      g = sourceIndex.size;
      sourceIndex.set(source, g);
    }
    nodeGroups[i] = g & 0xff;
  }
  return [...nodeGroups];
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
  return { manifest, cases, expect, pairs };
}

/** A result already in canonical form (the synthetic model, hashed or not). */
class Canonical {
  constructor(text) {
    this.text = text;
  }
}

function evaluator(oracle) {
  const graphs = new Map();
  const fresh = (name) => {
    const [nodes, edges] = graphs.get(name) ?? fail(`graph ${name} is not defined`);
    return [nodes.map(node), edges.map(edge)];
  };
  const indexed = (name) => oracle.indexModel(...fresh(name));
  const PATCH = ["addedNodes", "updatedNodes", "removedNodeIds", "addedEdges", "updatedEdges", "removedEdgeIds"];
  const run = {
    graph: (a) => {
      graphs.set(a.name, [a.nodes, a.edges]);
      return null;
    },
    makeRecordNodeId: (a) => oracle.makeRecordNodeId(a.source, a.databaseId, a.recordId),
    makeNoteNodeId: (a) => oracle.makeNoteNodeId(a.noteId),
    makeTagNodeId: (a) => oracle.makeTagNodeId(a.tagValue),
    makeEdgeId: (a) => oracle.makeEdgeId(a.source, a.target, a.kind, a.label, a.directed),
    parseNodeId: (a) => oracle.parseNodeId(a.nodeId),
    edgeKindFromType: (a) => oracle.edgeKindFromType(a.type ?? undefined),
    hashString: (a) => oracle.hashString(a.value),
    nodesEqual: (a) => oracle.nodesEqual(node(a.a), node(a.b)),
    edgesEqual: (a) => oracle.edgesEqual(edge(a.a), edge(a.b)),
    isEmptyPatch: (a) => oracle.isEmptyPatch(Object.fromEntries(PATCH.map((k, i) => [k, new Array(a.lengths[i]).fill(0)]))),
    emptyModel: () => wireModel(oracle.emptyModel()),
    applyDegreeWeights: (a) => {
      const [nodes, edges] = fresh(a.graph);
      oracle.applyDegreeWeights(nodes, edges);
      return nodes.map((n) => hex(n.weight));
    },
    indexModel: (a) => wireModel(indexed(a.graph)),
    diffGraph: (a) => wirePatch(oracle.diffGraph(indexed(a.previous), indexed(a.next))),
    deriveLegend: (a) => wireLegend(oracle.deriveLegend(indexed(a.graph))),
    neighborhood: (a) => [...oracle.neighborhood(indexed(a.graph), a.id, a.depth)],
    neighborhoodEdges: (a) => {
      const hood = oracle.neighborhoodEdges(indexed(a.graph), a.id, a.depth);
      return { nodeIds: [...hood.nodeIds], edgeIds: [...hood.edgeIds] };
    },
    buildSyntheticModel: (a) => {
      const text = canonical(wireModel(oracle.buildSyntheticModel(unhex(a.n))));
      return a.digest ? { sha256: sha256(text) } : new Canonical(text);
    },
    layoutGroups: (a) => layoutGroups(a.nodes),
  };
  return (fn, args) => {
    if (!Object.hasOwn(run, fn)) fail(`unknown function ${fn}`);
    const result = run[fn](args);
    return result instanceof Canonical ? result.text : canonical(result);
  };
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

/** H9: every differing group is ≥ 256 in graph-core and that value & 0xff in the oracle. */
function h9Explains(got, want) {
  const [oracle, core] = [JSON.parse(got), JSON.parse(want)];
  if (oracle.length !== core.length) return false;
  return core.every((g, i) => g === oracle[i] || (g >= 256 && oracle[i] === (g & 0xff)));
}

function compare({ cases, expect }, run) {
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
    if (fn === "layoutGroups" && Math.max(0, ...JSON.parse(expect[i])) >= 256) h9Crossed += 1;
    if (got === expect[i]) counts.equal += 1;
    else if (fn === "makeEdgeId" && h1Explains(args, got, expect[i])) {
      counts.declared += 1;
      h1Pairs.add(JSON.stringify([args.source, args.target]));
    } else if (fn === "layoutGroups" && h9Explains(got, expect[i])) counts.declared += 1;
    else {
      counts.unexplained += 1;
      mismatches.push({ line: i + 1, seed, fn, args, oracle: got, core: expect[i] });
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

async function main() {
  const fixtures = loadFixtures();
  const source = read(join(ROOT, "src", "core", "layout", "layoutBridge.ts"));
  const lines = new Set(source.split("\n").map((l) => l.trim()));
  for (const line of H9_SOURCE) if (!lines.has(line)) fail(`H9 transcription: layoutBridge.ts no longer has "${line}"`);
  const oracle = await publicSurface();
  const result = compare(fixtures, evaluator(oracle));
  const { manifest } = fixtures;
  const problems = checkDeclarations(fixtures.pairs, result.h1Pairs);
  const declaredH1 = result.functions.makeEdgeId?.declared ?? 0;
  if (declaredH1 < 3) problems.push(`H1: ${declaredH1} divergences observed, the fixture guarantees at least 3`);
  if (manifest.seeds >= 1000 && result.h9Crossed === 0) problems.push("H9: no layoutGroups case crossed 255 groups");
  for (const [fn, n] of Object.entries(manifest.counts)) {
    if (result.functions[fn]?.cases !== n) problems.push(`${fn}: manifest counts ${n} cases, ran ${result.functions[fn]?.cases ?? 0}`);
  }
  const unexplained = result.mismatches.length;
  const pass = unexplained === 0 && problems.length === 0;
  console.log(`oracle-diff: ${manifest.seeds} seeds, ${fixtures.cases.length} lines, node ${process.version}, icu ${process.versions.icu}`);
  for (const [fn, c] of Object.entries(result.functions).sort()) {
    console.log(`  ${fn.padEnd(20)} ${String(c.cases).padStart(6)} cases  ${String(c.equal).padStart(6)} equal  ${String(c.declared).padStart(5)} declared  ${c.unexplained} unexplained`);
  }
  console.log(`  H1 pairs observed diverging: ${result.h1Pairs.size} · H9 cases crossing 255 groups: ${result.h9Crossed}`);
  for (const m of result.mismatches.slice(0, 10)) {
    console.log(`  MISMATCH line ${m.line} seed ${m.seed} ${m.fn} ${clip(JSON.stringify(m.args))}\n    oracle ${clip(m.oracle)}\n    core   ${clip(m.core)}`);
  }
  for (const p of problems) console.log(`  PROBLEM ${p}`);
  const locale = Intl.DateTimeFormat().resolvedOptions().locale;
  const record = {
    gate: "oracle-diff",
    fingerprint: manifest.fingerprint,
    seeds: manifest.seeds,
    pass,
    runtime: { node: process.version, icu: process.versions.icu, locale },
    functions: result.functions,
    h1: { pairs: result.h1Pairs.size },
    h9: { crossed: result.h9Crossed },
  };
  mkdirSync(GATES, { recursive: true });
  writeFileSync(join(GATES, "oracle-diff.json"), `${JSON.stringify(record, null, 2)}\n`);
  console.log(pass ? "PASS" : `FAIL: ${unexplained} unexplained mismatches, ${problems.length} problems`);
  process.exit(pass ? 0 : 1);
}

try {
  await main();
} catch (error) {
  fail(error.stack ?? String(error));
}
