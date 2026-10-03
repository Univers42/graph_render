// The SDK's threaded motor (`crates/graph-sdk-js/src/threads.ts`) against its serial motor,
// through the published surface only: createMotor(threads artifact, { threads }) with Node
// worker_threads as helpers, then a live session per engine ticked to the same count on both.
// x, y and alpha must be byte-equal at every helper count.
//
//   node --experimental-strip-types harness/sdk-threads.mjs [--n 20000] [--helpers 1,3,6] [--break]
//
// --break passes threads.flags = 1 (the last part writes nothing): every cell must then differ,
// which also proves the helpers joined, since the pool runs one part per joined thread.
// --wasm and --threads name the artifacts. Exit 0 all equal, 1 a cell differs, 2 could not run.

import { readFile } from "node:fs/promises";
import { Worker } from "node:worker_threads";
import { createMotor } from "../crates/graph-sdk-js/src/index.ts";

const ENGINES = ["barnes_hut", "particle_mesh"];
const CALLS = 3;
const TICKS = 10;

function fail(message) {
  process.stderr.write(`sdk-threads: ${message}\n`);
  process.exit(2);
}

function parseArgs(argv) {
  const opts = { n: "20000", helpers: "1,3,6", break: false,
    wasm: "target/wasm32-unknown-unknown/release/graph_wasm.wasm",
    threads: "target/wasm-threads/wasm32-unknown-unknown/release/graph_wasm.wasm" };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--break") opts.break = true;
    else if (argv[i].startsWith("--") && i + 1 < argv.length) opts[argv[i].slice(2)] = argv[++i];
    else fail(`unknown argument ${argv[i]}`);
  }
  return opts;
}

/// A ring with one chord per node: connected, and every node has a neighbour far along it.
function ingestOf(n) {
  const node = (i) => ({ id: `n${i}`, kind: "record", database_id: null, source: "s", label: "", group: null,
    weight: 0.5, version: 0, has_note: false, icon: null });
  const edge = (id, a, b) => ({ id, source: `n${a}`, target: `n${b}`, kind: "relation", label: "",
    strength: 0.5, directed: false, record_id: null });
  const nodes = Array.from({ length: n }, (_, i) => node(i));
  const edges = [];
  for (let i = 0; i < n; i++) {
    edges.push(edge(`r${i}`, i, (i + 1) % n));
    const chord = (i * 7 + 3) % n;
    if (chord !== i && chord !== (i + 1) % n && (chord + 1) % n !== i) edges.push(edge(`c${i}`, i, chord));
  }
  return JSON.stringify({ version: 1, nodes, edges });
}

function spawn(start) {
  const helper = new Worker(new URL("./sdk-threads-helper.mjs", import.meta.url), { workerData: start });
  helper.on("error", (error) => fail(`a helper failed: ${error.message}`));
  helper.unref();
}

/// x, y and alpha after CALLS calls of tick(TICKS), as one byte string.
function settled(motor, ingest, engine) {
  const handle = motor.build(ingest);
  const session = motor.forceSession(handle, {}, engine);
  for (let i = 0; i < CALLS; i++) session.tick(TICKS);
  const { xs, ys } = session.positions();
  const bytes = (values) => Buffer.from(Float64Array.from(values).buffer).toString("hex");
  const out = `${bytes(xs)}|${bytes(ys)}|${bytes([session.alpha])}`;
  session.release();
  motor.release(handle);
  return out;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const ingest = ingestOf(Number(opts.n));
  const serial = await createMotor(await readFile(opts.wasm).catch((e) => fail(e.message)));
  if (!serial.available) fail(`the default artifact did not load from ${opts.wasm}`);
  const threadsBytes = await readFile(opts.threads).catch((e) => fail(`${e.message}: run scripts/orch/wasm-threads.sh`));
  const references = new Map(ENGINES.map((engine) => [engine, settled(serial, ingest, engine)]));
  let differing = 0;
  for (const helpers of opts.helpers.split(",").map(Number)) {
    const threads = { helpers, spawn, flags: opts.break ? 1 : 0 };
    const motor = await createMotor(threadsBytes, { threads });
    if (!motor.available) fail(`the threads artifact did not load with ${helpers} helpers`);
    for (const engine of ENGINES) {
      const same = settled(motor, ingest, engine) === references.get(engine);
      if (!same) differing++;
      console.log(`n ${opts.n} ${engine} helpers ${helpers}: ${same ? "equal" : "DIFFERS"}`);
    }
  }
  console.log(`${differing} differing cell(s)`);
  process.exit(differing === 0 ? 0 : 1);
}

await main();
