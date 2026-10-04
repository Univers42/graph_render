// Where the time of opening N synthetic nodes goes, on the real wasm under Node: the
// generator, the encode, the build, then a layout and toBytes, with the wasm memory after each.
//
//   ARM=json      (the default) syntheticRecords, JSON.stringify, motor.build
//   ARM=columns              syntheticColumns, assembleColumns, motor.buildColumns
//
// The two arms time *different* stages on purpose: the question is not which encoder is faster
// but what each path costs end to end, and `records`+`stringify` has no counterpart in the
// columns arm because there is no JSON text on it to build. `buildMs` and the MiB are the two
// numbers that are directly comparable.
//
// argv: n layout; ARM picks the arm; WASM picks the artifact.
// Run it with node-slim.sh (docs/measurements/perf-open-synth-columns.md).
//
// Caveat: one sample per stage, wall clock, one process per run; host load moves the times,
// never the MiB. Compare medians across several runs, never a single one.
import { assembleColumns, createMotor } from "../../crates/graph-sdk-js/src/index.ts";
import { readFile } from "node:fs/promises";
import { syntheticRecords } from "../../packages/graph-studio/src/source/synthetic.ts";
import { syntheticColumns } from "../../packages/graph-studio/src/source/synthetic-columns.ts";
const wasm = await readFile(process.env.WASM ?? "target/wasm32-unknown-unknown/release/graph_wasm.wasm");
const n = Number(process.argv[2]); const layout = process.argv[3];
const arm = process.env.ARM ?? "json";
let memory: WebAssembly.Memory | undefined;
const inst = WebAssembly.instantiate;
(WebAssembly as any).instantiate = async (...a: any[]) => { const r: any = await (inst as any)(...a); memory = (r.instance ?? r).exports.memory; return r; };
const motor = await createMotor(new Uint8Array(wasm));
const mib = () => Math.round((memory?.buffer.byteLength ?? 0) / 2 ** 20);
const spec = { seed: 1, nodeCount: n, degree: n >= 5000 ? 2 : 3, shape: "random" as const };
const row: Record<string, number | string> = { n, layout, arm };
let handle: ReturnType<typeof motor.build>;
let total = 0;
let a = performance.now();
if (arm === "columns") {
  const { rows, nodes, edgeCount } = syntheticColumns(spec);
  row.generateMs = Math.round(performance.now() - a); total += row.generateMs as number;
  row.nodeCount = nodes.length; row.edgeCount = edgeCount;
  a = performance.now(); const bytes = assembleColumns(rows);
  row.assembleMs = Math.round(performance.now() - a); total += row.assembleMs as number;
  row.documentMiB = Math.round(bytes.length / 2 ** 20);
  a = performance.now(); handle = motor.buildColumns(bytes);
  row.buildMs = Math.round(performance.now() - a); total += row.buildMs as number;
  row.buildMiB = mib();
} else {
  const { nodes, edges } = syntheticRecords(spec);
  row.generateMs = Math.round(performance.now() - a); total += row.generateMs as number;
  row.nodeCount = nodes.length; row.edgeCount = edges.length;
  a = performance.now(); const json = JSON.stringify({ version: 1, nodes, edges });
  row.encodeMs = Math.round(performance.now() - a); total += row.encodeMs as number;
  row.documentMiB = Math.round(json.length / 2 ** 20);
  a = performance.now(); handle = motor.build(json);
  row.buildMs = Math.round(performance.now() - a); total += row.buildMs as number;
  row.buildMiB = mib();
}
a = performance.now(); motor.layout(handle, layout); row.layoutMs = Math.round(performance.now() - a); row.layoutMiB = mib();
a = performance.now(); motor.toBytes(handle); row.bytesMs = Math.round(performance.now() - a); row.peakMiB = mib();
// The open path only: generate + encode + build. The layout and the snapshot below are the
// harness's own sanity stages, and neither is part of what a studio open pays before it can
// draw anything — at 1M the layout is twenty times the whole open.
row.totalMs = total;
console.log(JSON.stringify(row));
