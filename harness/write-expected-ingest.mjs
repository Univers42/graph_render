// One-off generator for `fixtures/ingest/expected-graph.json`'s `ingest` member. Kept
// out of the harness proper: a fixture must be readable and checkable without running
// its own generator, and the check is `harness/sdk-smoke.mjs --adapter-convergence`.
//
//   node --experimental-strip-types harness/write-expected-ingest.mjs [--out <path>]
//
// It writes only the `ingest` half, refusing if the file already has one, so a
// regeneration is always a deliberate act rather than a side effect of a test run.
// "Only": an existing `graph` member is carried through byte for byte — this file
// cannot derive a graph, and three runtimes pin the one that is there by
// `include_str!`/`readFile` (`crates/graph-core/src/ingest/tests/convergence.rs`,
// `crates/graph-wasm/src/contract/tests.rs`, `harness/sdk-smoke/convergence.mjs`).
// Regenerate that half with `GM_WRITE_INGEST_GRAPH=1 cargo test -p graph-core
// the_committed_graph`, not here.
//
// What this file is *not*: a check. It regenerates the `ingest` half with the very
// adapter it pins (`rows.ts`, through `adapter-convergence.mjs`), so by construction it
// cannot notice that the committed `ingest` half has drifted from the adapter — a
// generator agreeing with itself is not evidence. No gate row invokes it. The rows that
// do the comparing are `sdk-adapter-convergence` (the JS half) and `graph-core ingest`
// (the Rust half); this is the deliberate act behind them, and it is recorded here so
// nobody mistakes it for one of those.
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { canonicalJson, ingestFromRows } from "./adapter-convergence.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURE = join(HERE, "..", "fixtures", "ingest", "expected-graph.json");

/** Refuse to run: the reason, then how to run it. Exit 2, as the header promises. */
function refuse(reason) {
  process.stderr.write(`write-expected-ingest: ${reason}\n`);
  process.exit(2);
}

/** The `ingest` document as a member of the file: 2-space indented, the first line
 *  hanging off its key so the member still reads as one value. */
function ingestMember(ingest) {
  const text = JSON.stringify(JSON.parse(canonicalJson(ingest)), null, 2);
  return `"ingest": ${text.replace(/\n/g, "\n  ")}`;
}

/** The `graph` half as the three runtimes that pin it read it: an object with a
 *  `nodes` and an `edges` list, or `null` before that half has been derived. */
function graphIsCarried(graph) {
  if (graph === null) return true;
  return typeof graph === "object" && Array.isArray(graph.nodes) && Array.isArray(graph.edges);
}

const argv = process.argv.slice(2);
let out = FIXTURE;
for (let i = 0; i < argv.length; i += 1) {
  const arg = argv[i];
  if (arg === "--out") {
    if (argv[i + 1] === undefined) refuse("--out needs a path");
    out = argv[i + 1];
    i += 1;
  } else refuse(`unknown argument '${arg}'`);
}

const ingest = ingestMember(await ingestFromRows());

let existing = "";
try {
  existing = await readFile(out, "utf8");
} catch {
  // A missing file is the normal first run.
}
let parsed = null;
if (existing.length > 0) {
  try {
    parsed = JSON.parse(existing);
  } catch (err) {
    refuse(`${out} is not JSON (${err.message}); fix or remove it, nothing was written`);
  }
  if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
    refuse(`${out}'s root is not an object; nothing was written`);
  }
  if (parsed.ingest !== undefined) {
    refuse("`ingest` is already present; refusing to overwrite");
  }
  if (parsed.graph !== undefined && !graphIsCarried(parsed.graph)) {
    refuse("`graph` is present and is not the shape the pinned runtimes read (a `nodes` and an `edges` list, or null); refusing to write over it");
  }
}

// A file that already has a `graph` half gets the new member in front of it, so every
// byte of that half survives; a file without one is written whole, `graph` null.
if (parsed !== null && parsed.graph !== undefined) {
  if (!existing.startsWith("{")) refuse(`${out} does not start with '{'; nothing was written`);
  await writeFile(out, existing.replace(/^\{/, `{\n  ${ingest},`));
} else {
  await writeFile(out, `{\n  ${ingest},\n  "graph": null\n}\n`);
}
process.stdout.write(`wrote ${out} (ingest only; graph carried forward)\n`);