// The svc-sdk parity check, shared by `test/remote.test.mjs` (against a fake service) and
// `scripts/live-check.mjs` (against a running one): the same document and request run on
// the local wasm `Motor`, and every `ColumnId` of the remote result must hold the same kind,
// length and bytes as `Motor#column` for that run.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { ColumnId, createMotor } from "../src/index.ts";
import { rowsToIngest } from "../src/adapters/rows.ts";

const ROOT = new URL("../../../", import.meta.url);
export const WASM_PATH = fileURLToPath(new URL("target/wasm32-unknown-unknown/release/graph_wasm.wasm", ROOT));

/** Three fixtures, and every node kind, every edge kind and both dims between them. */
export const CASES = [
  { name: "forest, tree.tidy (Point, Polyline)", fixture: "fixtures/hierarchy/forest.json", shape: "studio",
    request: { layout: "layout.tree.tidy" } },
  { name: "hairball, packing.circle + bezier (Circle, Curve)", fixture: "fixtures/post/hairball.json", shape: "studio",
    request: { layout: "layout.packing.circle", post: "post.style.bezier" } },
  { name: "rows contract, treemap.squarified (Box, Line)", fixture: "fixtures/ingest/rows.json", shape: "rows",
    request: { layout: "layout.treemap.squarified", source: "contract" } },
  { name: "hairball, basic3d.sphere (3D, NodeZ)", fixture: "fixtures/post/hairball.json", shape: "studio",
    request: { layout: "layout.basic3d.sphere" } },
];

/** The wasm build, or a thrown error that says how to make it: a missing build is a failure,
 *  never a skip. */
export async function loadMotor(path = WASM_PATH) {
  let bytes;
  try {
    bytes = await readFile(path);
  } catch {
    throw new Error(`no wasm build at ${path}: run scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`);
  }
  return createMotor(bytes);
}

/** The edge `type` spelling the studio maps to `"hierarchy"` (`packages/graph-studio/src/source/ingest.ts`). */
function edgeKindOf(type) {
  const wire = String(type ?? "").toLowerCase();
  return wire.includes("hierarchy") || wire === "parent" || wire === "child_of" ? "hierarchy" : "relation";
}

/** A fixture's `{id, weight}` / `{id, source, target, type}` shorthand as the provisional
 *  ingest document, every member filled. */
function studioDocument(fixture) {
  const nodes = fixture.nodes.map((node) => ({
    id: node.id, kind: "record", database_id: null, source: "fixture", label: node.label ?? node.id,
    group: node.group ?? null, weight: node.weight ?? 0.5, version: 0, has_note: false, icon: null,
  }));
  const edges = fixture.edges.map((edge) => {
    const kind = edgeKindOf(edge.type);
    return {
      id: edge.id, source: edge.source, target: edge.target, kind, label: "", strength: edge.strength ?? 0.5,
      directed: kind === "hierarchy", record_id: null, child_first: false,
    };
  });
  return { version: 1, nodes, edges };
}

/** The document text a case sends, the same bytes to both arms. */
export async function documentOf(testCase) {
  const fixture = JSON.parse(await readFile(new URL(testCase.fixture, ROOT), "utf8"));
  return JSON.stringify(testCase.shape === "rows" ? rowsToIngest(fixture) : studioDocument(fixture));
}

/** Runs `request` on the local motor; the caller releases the handle. */
export function runLocal(motor, document, request) {
  const handle = request.source === "contract" ? motor.buildContract(document) : motor.build(document);
  const result = motor.layout(handle, request.layout);
  return { handle, result: request.post === undefined ? result : motor.post(handle, request.post) };
}

function sameBytes(left, right) {
  if (left.constructor !== right.constructor || left.length !== right.length) return false;
  const a = new Uint8Array(left.buffer, left.byteOffset, left.byteLength);
  const b = new Uint8Array(right.buffer, right.byteOffset, right.byteLength);
  return a.every((byte, i) => byte === b[i]);
}

/** Every way `snapshot` differs from the local run, as one line each; `[]` when none. */
export function differences(snapshot, motor, handle, result) {
  const found = [];
  for (const field of ["nodeKind", "edgeKind", "nodeCount", "dim"]) {
    if (snapshot[field] !== result[field]) found.push(`${field}: remote ${snapshot[field]}, local ${result[field]}`);
  }
  for (const [name, id] of Object.entries(ColumnId)) {
    const local = motor.column(handle, id);
    const remote = snapshot.column(id);
    if (local === null && remote === null) continue;
    if (local === null || remote === null) found.push(`${name}: remote ${remote === null ? "absent" : "present"}, local ${local === null ? "absent" : "present"}`);
    else if (!sameBytes(local, remote)) found.push(`${name}: ${remote.constructor.name}[${remote.length}] differs from ${local.constructor.name}[${local.length}]`);
  }
  return found;
}

/** `differences` for one case, with the local handle released whatever happens. */
export async function caseDifferences(remote, motor, testCase) {
  const document = await documentOf(testCase);
  const snapshot = await remote.layout(document, testCase.request);
  const { handle, result } = runLocal(motor, document, testCase.request);
  try {
    return differences(snapshot, motor, handle, result);
  } finally {
    motor.release(handle);
  }
}
