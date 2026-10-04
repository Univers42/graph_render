// The consumer of `/embed/<version>/graph-sdk.js`: the built SDK, driven exactly as a host that
// runs layouts without `<graph-studio>` would drive it.
//
//   scripts/orch/node-slim.sh node harness/sdk-bundle-smoke.mjs target/embed-bundle
//
// It loads the SERVED bytes, never `crates/graph-sdk-js/src`: the file under test is the one a host
// fetches, so a source-level smoke would pass on a build that never shipped. `embed-sdk` is that
// row; `negctl-embed-sdk` truncates `graph_wasm.wasm` in a copy and this must exit 1 naming the
// error, which is what `docs/reviews/review-bundle-unify.md` section 2 found missing: the file was
// built, staged, served, and consumed by nothing.
//
// The wasm source is a path, read as bytes, the way `harness/sdk-smoke.mjs:68` does
// (`const bytes = await readFile(wasmPath)`), because that is the published contract for
// `createMotor`: bytes of the module, not a URL it fetches. Here they come from the same
// directory as the SDK, so the pair under test is the pair a host downloads.
//
// The layout is the one the SDK's own registry names first (`motor.layouts()`, read the way
// `harness/sdk-smoke/layouts.mjs:11` reads it): a hard-coded `layout.grid` here would leave this
// file covering exactly one layout forever. The fixture is `fixtures/force/clustered.json`, 60
// nodes in 5 clusters, and its 60 positions are all asserted finite — a graph with no edges would
// satisfy "positions are finite" and prove nothing, so the fixture keeps its 138.
//
// Exit: 0 the built SDK laid the fixture out · 1 anything else, with the error's name. Nothing
// here is browser-only: the SDK's `createMotor` compiles the module and calls it, and the wasm
// module asks for no `window`, `document` or `Worker`.
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const FIXTURE = join(fileURLToPath(new URL("../", import.meta.url)), "fixtures/force/clustered.json");

/** The fixture as the provisional ingest document `motor.build` takes: the contract's ten node
 * members and eight edge members, filled the way `packages/graph-studio/src/source/ingest.ts`
 * fills the members a fixture leaves out. The fixture is written as ids and endpoints alone, so
 * this is the normalizer a host needs and the studio already ships — this file does not invent
 * shapes of its own. */
function ingestDocument(fixture) {
  const node = (value) => ({
    id: value.id,
    kind: "record",
    database_id: null,
    source: "file",
    label: value.label ?? value.id,
    group: null,
    weight: 0.5,
    version: 0,
    has_note: false,
    icon: null,
  });
  const edge = (value) => ({
    id: value.id,
    source: value.source,
    target: value.target,
    kind: "relation",
    label: "",
    strength: value.strength ?? 0.5,
    directed: false,
    record_id: null,
  });
  if (!Array.isArray(fixture.nodes) || !Array.isArray(fixture.edges)) {
    throw new TypeError("fixtures/force/clustered.json: no nodes/edges arrays");
  }
  return JSON.stringify({ version: 1, nodes: fixture.nodes.map(node), edges: fixture.edges.map(edge) });
}

/** Every position value of the run, read from the columns the run made readable.
 *
 * `NodeX` and `NodeY` are always readable after a run; `NodeZ` is not, on a 2D layout — the
 * column is absent rather than present-and-empty, and `motor.column` answers `null` for it. So
 * the absence is the run's own statement that the layout has no third axis, and reading through it
 * would make this file pass only on 3D layouts. What is not allowed is a readable axis that
 * carries no value per node. */
function positions(motor, handle, ids) {
  const values = [];
  const read = [];
  for (const id of ids) {
    const row = motor.column(handle, id);
    if (row === null) continue;
    const axis = Array.from(row, Number);
    if (axis.length !== motor.nodeCount(handle)) {
      throw new RangeError(`column ${id} has ${axis.length} values for ${motor.nodeCount(handle)} nodes`);
    }
    read.push(id);
    values.push(...axis);
  }
  if (read.length < 2) throw new RangeError(`only ${read.length} position axis/axes were readable`);
  return values;
}

async function main(dir) {
  const sdkUrl = pathToFileURL(join(dir, "graph-sdk.js")).href;
  const { createMotor, ColumnId } = await import(sdkUrl);
  if (typeof createMotor !== "function") {
    throw new TypeError(`${sdkUrl} exports no createMotor: it is not the motor's JS SDK`);
  }
  const bytes = await readFile(join(dir, "graph_wasm.wasm"));
  const motor = await createMotor(bytes);

  // The SDK's documented loader pattern (`docs/contract/wasm-abi.md`): a degraded motor answers
  // `available === false` and does not throw at create, and the latched load failure surfaces at
  // the first call that needs the module. So this asks for the registry rather than throwing its
  // own `Error`: the class that refused is `WasmUnavailableError`, and that name is the part of
  // the answer a host needs. On a load that succeeded, `layouts()` is the call this needed anyway.
  const registered = motor.layouts();

  const fixture = JSON.parse(await readFile(FIXTURE, "utf8"));
  const handle = motor.build(ingestDocument(fixture));
  if (!Array.isArray(registered) || registered.length === 0) {
    throw new Error("the SDK publishes no layout registry, so there is no default to run");
  }
  const layoutId = registered[0];
  const run = motor.layout(handle, layoutId);
  const values = positions(motor, handle, [ColumnId.NodeX, ColumnId.NodeY, ColumnId.NodeZ]);
  const bad = values.filter((value) => !Number.isFinite(value));

  console.log(`${dir}: ${layoutId} on fixtures/force/clustered.json`);
  console.log(`  ${run.nodeCount} nodes, ${run.nodeKind} nodes / ${run.edgeKind} edges, dim ${run.dim}`);
  console.log(`  ${values.length} position values read, ${bad.length} non-finite`);
  if (run.nodeCount !== fixture.nodes.length) {
    throw new Error(`the run placed ${run.nodeCount} nodes, the fixture has ${fixture.nodes.length}`);
  }
  if (bad.length > 0) {
    throw new RangeError(`${bad.length} of ${values.length} position values are not finite`);
  }
  console.log(`ok - the built SDK laid out ${run.nodeCount} nodes, every position finite`);
}

const [dir] = process.argv.slice(2).filter((arg) => !arg.startsWith("--"));
if (!dir) {
  console.error("usage: sdk-bundle-smoke.mjs <embed bundle DIR>");
  process.exit(1);
}
try {
  await main(dir);
} catch (error) {
  // The error's NAME, not only its message: a host's first question about a refusal is which
  // class refused, and `name` is the part the SDK contracts on (`WasmUnavailableError` and its
  // kin are documented by class, never by message text).
  const name = error instanceof Error ? error.name : typeof error;
  console.error(`sdk-bundle-smoke: ${name}: ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
}