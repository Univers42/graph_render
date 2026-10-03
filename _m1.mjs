import { readFile } from "node:fs/promises";
import { createMotor } from "./crates/graph-sdk-js/src/index.ts";

const bytes = await readFile("target/wasm32-unknown-unknown/release/graph_wasm.wasm");
const motor = await createMotor(bytes);
const node = (id) => ({ id, kind: "record", database_id: null, source: "s", label: id, group: null, weight: 0.5, version: 0, has_note: false, icon: null });
const edge = (id, s, t) => ({ id, source: s, target: t, kind: "relation", label: "", strength: 0.5, directed: false, record_id: null });
const INGEST = JSON.stringify({ version: 1, nodes: ["a", "b", "c", "d"].map(node), edges: [edge("e0", "a", "b"), edge("e1", "b", "c"), edge("e2", "c", "d")] });
const h = motor.build(INGEST);

const arg = Number(process.argv[2]);
const s = motor.forceSession(h);
console.log(`tick(${arg}) about to run ...`);
const t0 = process.hrtime.bigint();
const r = s.tick(arg);
const ms = Number(process.hrtime.bigint() - t0) / 1e6;
console.log(`tick(${arg}) -> status=${r.status} alpha=${r.alpha} ticksRun=${r.ticksRun} in ${ms.toFixed(1)}ms`);
s.release();
