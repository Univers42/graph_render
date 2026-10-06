// The wasm32 half of the transposition proof (`docs/decisions/dag-horizontal.md` condition 2):
// the same two runs the native tests make, made here against the compiled module, so D10's
// bit-identical native vs wasm32 is held by the flag and not only by the default run.
//
//   node --experimental-strip-types harness/horizontal-wasm.mjs <graph_wasm.wasm> [--break]
//
// For each dag fixture and each layered layout: run at `horizontal: false`, run at
// `horizontal: true`, and require the second to be the first with x and y exchanged — every
// node centre and every edge interior point, the CSR offsets untouched. Compared as
// `Uint32Array` views of the `Float32Array`s, so `-0.0` and `0.0` are different answers.
//
// `--break` sends `false` for the second run too, so the two drawings are equal and every case
// must FAIL: the control that shows the comparison above is the real one and not a formality.
//
// Exit: 0 every case passed · 1 a case failed · 2 could not run.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { ColumnId, createMotor } from "../crates/graph-sdk-js/src/index.ts";

const FIXTURES = ["multi-span", "wide-layer"].map((name) =>
  join(fileURLToPath(new URL("../", import.meta.url)), "fixtures/dag", `${name}.json`));
const LAYOUTS = ["layout.dag.sugiyama", "layout.dag.lanes"];

/** The fixture as the provisional ingest document `motor.build` takes: the contract's ten node
 *  members and eight edge members, filled the way `harness/sdk-bundle-smoke.mjs` fills them. */
function ingest(fixture) {
  const node = (value) => ({
    id: value.id, kind: "record", database_id: null, source: "file",
    label: value.label ?? value.id, group: null, weight: 0.5, version: 0,
    has_note: false, icon: null,
  });
  const edge = (value) => ({
    id: value.id, source: value.source, target: value.target, kind: "relation",
    label: "", strength: value.strength ?? 0.5, directed: false, record_id: null,
  });
  return JSON.stringify({ version: 1, nodes: fixture.nodes.map(node), edges: fixture.edges.map(edge) });
}

/** One column as the bits the wire carries, copied out: a `Column` aliases the motor's memory
 *  and is invalidated by the next call on it, so the two runs cannot be read after both. */
function bits(column) {
  return Uint32Array.from(new Uint32Array(column.buffer, column.byteOffset, column.length));
}

/** The two columns and the two path columns of the handle's last run. */
function drawing(motor, handle) {
  return {
    x: bits(motor.column(handle, ColumnId.NodeX)),
    y: bits(motor.column(handle, ColumnId.NodeY)),
    offsets: Uint32Array.from(motor.column(handle, ColumnId.EdgeOffsets)),
    pts: bits(motor.column(handle, ColumnId.EdgePts)),
  };
}

/** The first place `turned` is not `flat` with x and y exchanged, or null. */
function firstMismatch(flat, turned) {
  if (turned.x.length !== flat.y.length || turned.y.length !== flat.x.length) {
    return `node columns ${flat.x.length}/${flat.y.length} against ${turned.x.length}/${turned.y.length}`;
  }
  for (let i = 0; i < turned.x.length; i += 1) {
    if (turned.x[i] !== flat.y[i]) return `node ${i} x is not the vertical y`;
    if (turned.y[i] !== flat.x[i]) return `node ${i} y is not the vertical x`;
  }
  if (turned.offsets.length !== flat.offsets.length) {
    return `offsets ${flat.offsets.length} against ${turned.offsets.length}`;
  }
  for (let i = 0; i < turned.offsets.length; i += 1) {
    if (turned.offsets[i] !== flat.offsets[i]) return `offset ${i} moved`;
  }
  if (turned.pts.length !== flat.pts.length) {
    return `path points ${flat.pts.length} against ${turned.pts.length}`;
  }
  for (let p = 0; p < turned.pts.length; p += 2) {
    if (turned.pts[p] !== flat.pts[p + 1]) return `point ${p / 2} x is not the vertical y`;
    if (turned.pts[p + 1] !== flat.pts[p]) return `point ${p / 2} y is not the vertical x`;
  }
  return null;
}

async function main(wasmPath, broken) {
  const motor = await createMotor(await readFile(wasmPath));
  if (!motor.available) throw new Error("the module did not load, so nothing could be measured against it");
  let failed = 0;
  for (const fixture of FIXTURES) {
    const name = fixture.split("/").pop().replace(/\.json$/, "");
    const handle = motor.build(ingest(JSON.parse(await readFile(fixture, "utf8"))));
    for (const layout of LAYOUTS) {
      motor.run(handle, layout, { params: { horizontal: false } });
      const flat = drawing(motor, handle);
      motor.run(handle, layout, { params: { horizontal: !broken } });
      const turned = drawing(motor, handle);
      const mismatch = firstMismatch(flat, turned);
      if (mismatch === null) {
        console.log(`PASS ${name} ${layout}`);
      } else {
        failed += 1;
        console.log(`FAIL ${name} ${layout} ${mismatch}`);
      }
    }
    motor.release(handle);
  }
  return failed;
}

const args = process.argv.slice(2);
const broken = args.includes("--break");
const [wasmPath] = args.filter((arg) => !arg.startsWith("--"));
if (!wasmPath) {
  console.error("usage: horizontal-wasm.mjs <graph_wasm.wasm> [--break]");
  process.exit(2);
}
try {
  process.exitCode = (await main(wasmPath, broken)) === 0 ? 0 : 1;
} catch (error) {
  console.error(`horizontal-wasm: ${error && error.message ? error.message : String(error)}`);
  process.exitCode = 2;
}
