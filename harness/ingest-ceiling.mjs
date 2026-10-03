// The ingest byte ceiling, measured (F-16, `docs/decisions/wasm-ingest-limits.md` steps 1-2):
// one document per run, written to `target/`, then handed to `gm_build` through the real
// wasm32 release artifact. Prints one row: `nodes edges degree bytes outcome`.
//
//   node --experimental-strip-types harness/ingest-ceiling.mjs --wasm <path> --nodes N --degree D
//   ... --pad BYTES      prefix that many spaces (leading JSON whitespace, RFC 8259) to reach
//                        an exact byte count; the parser reads the same document either way
//   ... --shape vault     the studio's other model; `random` by default
//   ... --out-dir DIR     target/ingest-ceiling by default
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
// not run (no artifact, no room to write).

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

/** `gm_alloc`, copy, `gm_build`, `gm_free`: the same three calls a host makes (C5, C7). */
async function build(wasmPath, file) {
  const bytes = await readFile(wasmPath);
  const exports = await loadMotor(bytes);
  const size = (await stat(file)).size;
  const ptr = exports.gm_alloc(size);
  if (ptr === 0) return { outcome: "alloc-refused", code: exports.gm_last_error() };
  new Uint8Array(exports.memory.buffer, ptr, size).set(await readFile(file));
  const handle = exports.gm_build(ptr, size);
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
  const result = await build(spec.wasm, file);
  process.stdout.write(
    `${counted.nodes} ${counted.edges} ${spec.degree} ${written} ${result.outcome} code=${result.code}\n`,
  );
  process.exit(result.outcome === "built" ? 0 : 1);
}

await main();