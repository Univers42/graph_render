// Times build, layout and toBytes on the real wasm under Node and reports the wasm memory after
// each. argv: n layout; WASM picks the artifact. Run it with node-slim.sh (docs/measurements/perf-heap.md).
//
// Caveat: one sample per stage, wall clock; host load moves the times, never the MiB.
import { createMotor } from "../../crates/graph-sdk-js/src/index.ts";
import { readFile } from "node:fs/promises";
import { syntheticRecords } from "../../packages/graph-studio/src/source/synthetic.ts";
const wasm = await readFile(process.env.WASM ?? "target/wasm32-unknown-unknown/release/graph_wasm.wasm");
const n = Number(process.argv[2]); const layout = process.argv[3];
let memory: WebAssembly.Memory | undefined;
const inst = WebAssembly.instantiate;
(WebAssembly as any).instantiate = async (...a: any[]) => { const r: any = await (inst as any)(...a); memory = (r.instance ?? r).exports.memory; return r; };
const motor = await createMotor(new Uint8Array(wasm));
const mib = () => Math.round((memory?.buffer.byteLength ?? 0) / 2 ** 20);
const { nodes, edges } = syntheticRecords({ seed: 1, nodeCount: n, degree: n >= 5000 ? 2 : 3, shape: "random" });
const json = JSON.stringify({ version: 1, nodes, edges });
const row: Record<string, number | string> = { n, layout, jsonMiB: Math.round(json.length / 2 ** 20) };
let a = performance.now(); const h = motor.build(json); row.buildMs = Math.round(performance.now() - a); row.buildMiB = mib();
a = performance.now(); motor.layout(h, layout); row.layoutMs = Math.round(performance.now() - a); row.layoutMiB = mib();
a = performance.now(); motor.toBytes(h); row.bytesMs = Math.round(performance.now() - a); row.peakMiB = mib();
console.log(JSON.stringify(row));
