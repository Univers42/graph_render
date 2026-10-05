#!/usr/bin/env node
// bench.mjs <wasm> <log>...: the plugin's in-process steps, timed, 3 rounds, medians. Per log:
// parse, map + rowsToIngest, stringify, the wasm motor's contract build (the same derivation
// as graph-cli ingest), then `layout.dag.sugiyama` and `layout.dag.lanes` (when registered) with their note counts.
// For `layout.dag.lanes` it also prints `layout.dag.lanes.width=<k>`, the number of distinct node
// `x` values — the lane count, read off `geometry.nodes.x`, the flat f32 column of the JSON face's
// Point node geometry (`geometry.nodes.kind` is "Point"; `nodes.id` is the parallel id column).
// Caveat: wall clock on a shared host; a median under load is an upper bound. Container
// start-up (node-slim, gr) is outside every number here and is reported apart.
import { readFile } from "node:fs/promises";
import { rowsToIngest } from "../../../crates/graph-sdk-js/src/adapters/rows.ts";
import { createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { parseLog, toRows } from "./map.mjs";

const ROUNDS = 3;
const [wasmPath, ...logs] = process.argv.slice(2);
if (!wasmPath || logs.length === 0) {
  process.stderr.write("usage: bench.mjs <wasm> <log>...\n");
  process.exit(2);
}
const motor = await createMotor(await readFile(wasmPath));
// `createMotor` never throws: a load failure resolves to a degraded motor whose every method
// refuses (`Motor.available`). Timing that would print an upper bound of nothing.
if (!motor.available) {
  process.stderr.write(`bench: ${wasmPath}: the motor refused to load; nothing to time\n`);
  process.exit(2);
}
// dag.dot is left out: 2.6 s at 2k commits in the probe, so it would dominate every round.
const layouts = motor.layouts().filter((id) => ["layout.dag.sugiyama", "layout.dag.lanes"].includes(id));
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const timed = (f) => { const t = performance.now(); const value = f(); return [value, performance.now() - t]; };

for (const path of logs) {
  const text = await readFile(path, "utf8");
  const state = { times: new Map(), refused: new Map() };
  let facts = "";
  for (let round = 0; round < ROUNDS; round += 1) facts = once(text, state);
  const cells = [...state.times].map(([key, ms]) => `${key}=${median(ms).toFixed(1)}ms`);
  const refused = [...state.refused].map(([id, why]) => `${id}=REFUSED ${why}`);
  console.log(`${path} ${facts} ${[...cells, ...refused].join(" ")} load=${(await readFile("/proc/loadavg", "utf8")).split(" ")[0]}`);
}

function once(text, state) {
  const push = (key, ms) => state.times.set(key, [...(state.times.get(key) ?? []), ms]);
  const [commits, parse] = timed(() => parseLog(text));
  const [ingest, map] = timed(() => rowsToIngest(toRows(commits, "bench").rows));
  const [json, stringify] = timed(() => JSON.stringify(ingest));
  const [handle, build] = timed(() => motor.buildContract(json));
  [["parse", parse], ["map", map], ["stringify", stringify], ["build", build]].forEach(([k, ms]) => push(k, ms));
  const notes = layouts.map((id) => layoutOnce(handle, id, push, state));
  motor.release(handle);
  return `n=${commits.length} bytes=${Buffer.byteLength(json)} ${notes.join(" ")}`;
}

// A layout that refuses (a scale ceiling, a param out of range) is recorded as a refusal and
// the round goes on: the other numbers are still measured, and no median is invented for it.
function layoutOnce(handle, id, push, state) {
  let run;
  let ms;
  try {
    [run, ms] = timed(() => motor.layout(handle, id));
  } catch (error) {
    state.refused.set(id, error.message);
    return `${id}=REFUSED`;
  }
  push(id, ms);
  const json = JSON.parse(motor.toJSON(run.handle));
  const codes = {};
  for (const code of json.notes?.code ?? []) codes[code] = (codes[code] ?? 0) + 1;
  const width = id === "layout.dag.lanes" ? ` layout.dag.lanes.width=${laneWidth(json)}` : "";
  return `${id}.notes=${JSON.stringify(codes)}${width}`;
}

// The lanes width: the number of distinct node `x` values, read off the JSON face's Point node
// column `geometry.nodes.x`. Node y is not read — a lane is a column, a row is a rank.
function laneWidth(json) {
  return new Set(json.geometry.nodes.x).size;
}