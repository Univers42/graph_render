// The ingest byte ceiling, measured (F-16, `docs/decisions/wasm-ingest-limits.md` steps 1-2):
// one document per run, written to `target/`, then handed to `gm_build` through the real
// wasm32 release artifact. Prints one row: `nodes edges degree bytes outcome`.
//
//   node --experimental-strip-types harness/ingest-ceiling.mjs --wasm <path> --nodes N --degree D
//   ... --pad BYTES      prefix that many spaces (leading JSON whitespace, RFC 8259) to reach
//                        an exact byte count; the parser reads the same document either way
//   ... --shape vault     the studio's other model; `random` by default
//   ... --out-dir DIR     target/ingest-ceiling by default
//   ... --phases          also print the per-phase linear-memory marks, for an artifact
//                        built with `--features probe` (see `crates/graph-wasm/src/ingest/phases.rs`).
//                        The marks are read out of `memory.buffer`, not by calling the
//                        module, so they survive the trap they were taken to explain.
//
// One spec per process on purpose: the driver bounds each run with `timeout`, and a sweep
// inside one process could not be bounded per document. Documents come from the studio's own
// generator (`packages/graph-studio/src/source/synthetic.ts`), the producer at the 1M-node
// scale target the decision record names, so the density here is the real one. The JSON is
// streamed record by record instead of through `JSON.stringify` on the whole document, which
// is byte-for-byte the same string (`--verify` proves the two lengths agree on a small spec)
// and does not need the whole text resident twice.
//
// Exit codes follow graph-cli: 0 built, 1 the module refused the document by name, 2 could
// not run (no artifact, no room to write), 3 the module trapped (`unreachable`) — which is
// what the ceiling exists to replace, so it gets a code of its own rather than reading as
// a refusal.

import { createWriteStream } from "node:fs";
import { mkdir, readFile, stat } from "node:fs/promises";
import { once } from "node:events";
import { loadMotor } from "../crates/graph-sdk-js/src/wasm.ts";
import { syntheticIngest, syntheticRecords } from "../packages/graph-studio/src/source/synthetic.ts";

function parseArgs(argv) {
  const spec = { seed: 0, degree: 4, shape: "random", pad: 0, outDir: "target/ingest-ceiling" };
  for (let i = 0; i < argv.length; i += 1) {
    const flag = argv[i];
    const value = argv[i + 1];
    if (flag === "--wasm") { spec.wasm = value; i += 1; }
    else if (flag === "--nodes") { spec.nodes = Number(value); i += 1; }
    else if (flag === "--degree") { spec.degree = Number(value); i += 1; }
    else if (flag === "--seed") { spec.seed = Number(value); i += 1; }
    else if (flag === "--shape") { spec.shape = value; i += 1; }
    else if (flag === "--pad") { spec.pad = Number(value); i += 1; }
    else if (flag === "--out-dir") { spec.outDir = value; i += 1; }
    else if (flag === "--phases") { spec.phases = true; }
    else if (flag === "--verify") { spec.verify = true; }
    else fail(`unknown flag ${flag}`);
  }
  if (!spec.wasm) fail("usage: ingest-ceiling.mjs --wasm <path> --nodes N [--degree D] [--pad BYTES]");
  if (!Number.isInteger(spec.nodes) || spec.nodes < 2) fail("--nodes must be an integer >= 2");
  return spec;
}

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(2);
}

/** The exact bytes `JSON.stringify` would have written for the whole document, counted
 * without building it: two fixed wrappers and the records joined by one comma. */
function documentBytes(records) {
  const head = '{"version":1,"nodes":[';
  const middle = '],"edges":[';
  const tail = "]}";
  let total = head.length + middle.length + tail.length;
  const counted = (list) => list.reduce((sum, item, index) => sum + JSON.stringify(item).length + (index ? 1 : 0), 0);
  return head.length + counted(records.nodes) + middle.length + counted(records.edges) + tail.length;
}

/** The document to `file`, record by record, and the byte count the parser will see. */
async function emit(spec, file) {
  const records = syntheticRecords({ seed: spec.seed, nodeCount: spec.nodes, degree: spec.degree, shape: spec.shape });
  const expected = documentBytes(records) + spec.pad;
  const stream = createWriteStream(file);
  const sink = async (text) => { if (!stream.write(text)) await once(stream, "drain"); };
  await sink(" ".repeat(spec.pad));
  await sink('{"version":1,"nodes":[');
  for (const [index, node] of records.nodes.entries()) await sink(`${index ? "," : ""}${JSON.stringify(node)}`);
  await sink('],"edges":[');
  for (const [index, edge] of records.edges.entries()) await sink(`${index ? "," : ""}${JSON.stringify(edge)}`);
  await sink("]}");
  stream.end();
  await once(stream, "finish");
  return { expected, nodes: records.nodes.length, edges: records.edges.length };
}

/** The phase ids `crates/graph-wasm/src/ingest/phases.rs` records, in record order. */
const PHASE_NAMES = ["copy", "parse", "records", "index_model", "returned", "arena_text", "columns", "csrs"];

/** `(phase, bytes)` pairs out of the module's mark table, read from `memory.buffer`.
 *
 * The address comes from `gm_probe_base` and is a `static` in linear memory, so this works
 * on an instance that already trapped: the wasm memory is still there, only the instance's
 * call surface is gone. A module without the probe feature has no `gm_probe_base` and no
 * marks to read. */
function readMarks(exports) {
  if (typeof exports.gm_probe_base !== "function") return null;
  const table = new Uint32Array(exports.memory.buffer, exports.gm_probe_base(), 64);
  const marks = new Map();
  for (let i = 1; i + 1 <= table[0]; i += 2) marks.set(table[i], table[i + 1]);
  return marks;
}

function reportPhases(marks, outcome) {
  if (marks === null) return;
  const rows = [...marks].map(([id, bytes]) => `  ${PHASE_NAMES[id] ?? id} ${bytes}`);
  process.stdout.write(`phases ${outcome}: ${rows.length ? "\n" + rows.join("\n") : "none recorded"}\n`);
}

/** `gm_alloc`, copy, `gm_build`, `gm_free`: the same three calls a host makes (C5, C7). */
async function build(wasmPath, file, phases) {
  const bytes = await readFile(wasmPath);
  const exports = await loadMotor(bytes);
  const size = (await stat(file)).size;
  const ptr = exports.gm_alloc(size);
  if (ptr === 0) return { outcome: "alloc-refused", code: exports.gm_last_error() };
  new Uint8Array(exports.memory.buffer, ptr, size).set(await readFile(file));
  if (phases && typeof exports.gm_probe_reset === "function") exports.gm_probe_reset();
  let handle = 0;
  let trapped = null;
  try {
    handle = exports.gm_build(ptr, size);
  } catch (error) {
    trapped = error;
  }
  if (phases) reportPhases(readMarks(exports), trapped ? `trapped ${trapped.message ?? trapped}` : "returned");
  if (trapped) return { outcome: "trapped", code: exports.gm_last_error(), handle: 0 };
  const code = exports.gm_last_error();
  exports.gm_free(ptr, size);
  return { outcome: handle === 0 ? "refused" : "built", code, handle };
}

async function main() {
  const spec = parseArgs(process.argv.slice(2));
  if (spec.verify) {
    const one = syntheticIngest({ seed: spec.seed, nodeCount: spec.nodes, degree: spec.degree, shape: spec.shape });
    process.stdout.write(`verify ${spec.nodes} nodes: ${one.length} bytes from JSON.stringify\n`);
  }
  await mkdir(spec.outDir, { recursive: true });
  const name = `${spec.shape}-n${spec.nodes}-d${spec.degree}-p${spec.pad}.json`;
  const file = `${spec.outDir}/${name}`;
  const counted = await emit(spec, file);
  const written = (await stat(file)).size;
  if (written !== counted.expected) fail(`streamed ${written} bytes, expected ${counted.expected}`);
  const result = await build(spec.wasm, file, spec.phases);
  process.stdout.write(
    `${counted.nodes} ${counted.edges} ${spec.degree} ${written} ${result.outcome} code=${result.code}\n`,
  );
  process.exit(result.outcome === "built" ? 0 : result.outcome === "trapped" ? 3 : 1);
}

await main();