// A third party's own smoke test of the published SDK: imports only
// `crates/graph-sdk-js/src/index.ts`'s public surface — never a raw wasm export, never
// `wasm.ts`/`views.ts` directly, the same way a real consumer would. Exercises a full
// build/layout/column/toJSON/release round trip, **every layout the module's own registry
// names** (phase step 7: "runs each gated layout, and prints node counts and bounds" — the
// list comes from `Motor#layouts`, i.e. `gm_layout_count`/`gm_layout_id`, never a literal
// name, so a layout registered after this script was written is covered with no edit here),
// the contract's own column-presence table restated kind by kind (C3: the reserved Circle
// `r` / Box `w,h` and Polyline columns), a NaN written through a zero-copy column view
// refusing at `toJSON` (C8's D9 re-validation), a released handle staying refused rather
// than silently answered (C6), and `options` acceptance (C16).
//
//   node --experimental-strip-types harness/sdk-smoke.mjs <graph_wasm.wasm>
//   node --experimental-strip-types harness/sdk-smoke.mjs --adapter-convergence
//
// Exit codes follow graph-cli: 0 pass, 1 ran and failed, 2 could not run.

import { readFile } from "node:fs/promises";
import {
  AnalysisRefusedError,
  ColumnId,
  InvalidOptionsError,
  PostRefusedError,
  TamperedGeometryError,
  WasmUnavailableError,
  createMotor,
  resetForTests,
} from "../crates/graph-sdk-js/src/index.ts";
import { canonicalJson, expectedIngest, ingestFromNotion, ingestFromRows } from "./adapter-convergence.mjs";

function fail(message) {
  process.stderr.write(`sdk-smoke: could not run: ${message}\n`);
  process.exit(2);
}

const args = process.argv.slice(2);
const convergenceOnly = args.includes("--adapter-convergence");
const [wasmPath] = args.filter((arg) => !arg.startsWith("--"));

let failures = 0;
function check(name, condition) {
  if (condition) {
    process.stdout.write(`ok - ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`not ok - ${name}\n`);
  }
}

// The phase's proof, in the one runtime that can run the adapters: the same logical
// dataset in two source shapes, mapped by two independent adapters, must produce one
// contract document — byte for byte. The other half of the proof (that document derives
// `expected-graph.json`'s graph) is Rust, in `crates/graph-core/src/ingest/tests.rs`,
// because the derivation lives there and only there. Both halves read the same
// committed file, so the two runtimes are pinned to one artifact rather than to two
// that can drift.
if (convergenceOnly) {
  const fromRows = canonicalJson(await ingestFromRows());
  const fromNotion = canonicalJson(await ingestFromNotion());
  const expected = canonicalJson(await expectedIngest());
  check("the rows adapter maps fixtures/ingest/rows.json", fromRows.length > 0);
  check("the notion adapter maps fixtures/ingest/notion.json", fromNotion.length > 0);
  check("two adapters, one contract document, identical bytes", fromRows === fromNotion);
  check("both adapters produce the document expected-graph.json pins", fromRows === expected);
  reportDifference(fromRows, { notion: fromNotion, expected });
  process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
  process.exit(failures === 0 ? 0 : 1);
}

if (!wasmPath) fail("usage: sdk-smoke.mjs <wasm> | --adapter-convergence");

async function refusedWith(errorClass, run) {
  try {
    await run();
    return false;
  } catch (error) {
    return error instanceof errorClass;
  }
}

/** Where two canonical documents first differ, and the bytes around it. A convergence
 * failure with no position in it is a failure a reader has to bisect by hand, which is
 * the opposite of what a gate row should hand them. */
function reportDifference(left, others) {
  for (const [name, other] of Object.entries(others)) {
    if (left === other) continue;
    const at = firstDifference(left, other);
    process.stdout.write(
      `#   rows vs ${name}: first difference at byte ${at}\n` +
        `#     rows    ${JSON.stringify(left.slice(Math.max(0, at - 48), at + 48))}\n` +
        `#     ${name.padEnd(7)}${JSON.stringify(other.slice(Math.max(0, at - 48), at + 48))}\n`,
    );
  }
}

function firstDifference(a, b) {
  const limit = Math.min(a.length, b.length);
  for (let i = 0; i < limit; i += 1) {
    if (a[i] !== b[i]) return i;
  }
  return limit;
}

// `docs/contract/wasm-abi.md` "Columns", restated here the way a third-party consumer
// reads it: which node/edge column ids apply to which geometry kind (`null` = every kind; a
// string or a list of names = those kinds only). Restated rather than imported from the
// SDK (`views.ts`'s own `columnApplies`), because a consumer checking the SDK against the
// SDK would agree with any mistake the SDK makes.
const NODE_COLUMN_KINDS = new Map([
  [ColumnId.NodeX, null],
  [ColumnId.NodeY, null],
  [ColumnId.NodeR, "Circle"],
  [ColumnId.NodeW, "Box"],
  [ColumnId.NodeH, "Box"],
]);
const EDGE_COLUMN_KINDS = new Map([
  [ColumnId.EdgeSource, null],
  [ColumnId.EdgeTarget, null],
  [ColumnId.EdgeOffsets, ["Polyline", "Curve"]],
  [ColumnId.EdgePts, ["Polyline", "Curve"]],
  [ColumnId.EdgeCurveDegree, ["Curve"]],
]);

function appliesTo(kind, wanted) {
  if (wanted === null) return true;
  return Array.isArray(wanted) ? wanted.includes(kind) : wanted === kind;
}

function boundsOf(motor, handle) {
  const span = (column) => {
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (const value of column) {
      min = Math.min(min, value);
      max = Math.max(max, value);
    }
    return { min, max };
  };
  const xs = span(motor.column(handle, ColumnId.NodeX));
  const ys = span(motor.column(handle, ColumnId.NodeY));
  return { minX: xs.min, maxX: xs.max, minY: ys.min, maxY: ys.max };
}

// Everything the contract's table says about one layout's run over this handle, as a list
// of problems (empty when the run agrees with it). There is no edge-count export, so `m` is
// read off the source column: the table's rule is that every edge column shares one `m`.
function columnProblems(motor, handle, layoutId, run) {
  const problems = [];
  const source = motor.column(handle, ColumnId.EdgeSource);
  const target = motor.column(handle, ColumnId.EdgeTarget);
  const offsets = motor.column(handle, ColumnId.EdgeOffsets);
  const m = source === null ? 0 : source.length;
  for (const [id, wanted] of NODE_COLUMN_KINDS) {
    const column = motor.column(handle, id);
    if (!appliesTo(run.nodeKind, wanted)) {
      if (column !== null) problems.push(`node column ${id} is present but does not apply to ${run.nodeKind}`);
      continue;
    }
    if (column === null) problems.push(`node column ${id} is absent, but applies to ${run.nodeKind}`);
    else if (column.length !== run.nodeCount) {
      problems.push(`node column ${id} has ${column.length} elements, not the node count ${run.nodeCount}`);
    }
  }
  for (const [id, wanted] of EDGE_COLUMN_KINDS) {
    const column = motor.column(handle, id);
    if (!appliesTo(run.edgeKind, wanted)) {
      if (column !== null) problems.push(`edge column ${id} is present but does not apply to ${run.edgeKind}`);
    } else if (column === null) {
      problems.push(`edge column ${id} is absent, but applies to ${run.edgeKind}`);
    }
  }
  if (source === null || target === null) {
    problems.push("the edge source/target columns are absent, but apply to every edge kind");
  } else if (source.length !== target.length) {
    problems.push(`edge source has ${source.length} elements and edge target ${target.length}`);
  }
  if (offsets !== null) {
    if (offsets.length !== m + 1) problems.push(`edge offsets has ${offsets.length} elements, not m + 1 = ${m + 1}`);
    const points = motor.column(handle, ColumnId.EdgePts);
    const want = 2 * offsets[offsets.length - 1];
    if (points === null) problems.push("edge points are absent where edge offsets are present");
    else if (points.length !== want) problems.push(`edge points has ${points.length} elements, not 2*offsets[m] = ${want}`);
  }
  const degree = motor.column(handle, ColumnId.EdgeCurveDegree);
  if (degree !== null && degree.length !== 1) problems.push(`edge curve degree has ${degree.length} elements, not 1`);
  for (const id of [ColumnId.NoteCode, ColumnId.NoteIndex]) {
    if (motor.column(handle, id) !== null) {
      problems.push(`reserved note column ${id} is present; it stays reserved until the notes section lands`);
    }
  }
  return problems.map((problem) => `${layoutId}: ${problem}`);
}

const bytes = await readFile(wasmPath);

// C16: options this phase accept only {} and { exec: "auto" }; anything else is refused
// before the module is even asked to load.
await createMotor(bytes, {});
await createMotor(bytes, { exec: "auto" });
check("an unknown options key is refused", await refusedWith(InvalidOptionsError, () => createMotor(bytes, { bogus: true })));
check('options.exec other than "auto" is refused', await refusedWith(InvalidOptionsError, () => createMotor(bytes, { exec: "gpu" })));

const motor = await createMotor(bytes);

const node = (id, kind) => ({
  id,
  kind,
  database_id: null,
  source: "s",
  label: id.toUpperCase(),
  group: null,
  weight: 0.5,
  version: 0,
  has_note: false,
  icon: null,
});
const ingest = JSON.stringify({
  version: 1,
  nodes: [node("a", "record"), node("b", "note")],
  edges: [{ id: "e", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null }],
});

const handle = motor.build(ingest);
check("build reports the right node count", motor.nodeCount(handle) === 2);

// Every layout the module registered, read once through the SDK's own view of the
// registry (C1). A hard-coded name here would make this file cover exactly one layout
// forever: p3's four would arrive, the smoke test would keep passing, and none of them
// would ever have been run through the published SDK.
const registered = typeof motor.layouts === "function" ? motor.layouts() : [];
check("the SDK publishes the module's layout registry (C1)", registered.length > 0);
check(
  "the registry names layout.grid and repeats no id",
  registered.includes("layout.grid") && new Set(registered).size === registered.length,
);

let ran = 0;
for (const layoutId of registered) {
  const result = motor.layout(handle, layoutId);
  ran += 1;
  const bounds = boundsOf(motor, handle);
  process.stdout.write(
    `# ${layoutId}: ${result.nodeCount} nodes, ${result.nodeKind} nodes / ${result.edgeKind} edges, ` +
      `bounds x[${bounds.minX}, ${bounds.maxX}] y[${bounds.minY}, ${bounds.maxY}]\n`,
  );
  check(`${layoutId}: every node is placed`, result.nodeCount === 2);
  check(
    `${layoutId}: its bounds are finite and not a single point`,
    [bounds.minX, bounds.maxX, bounds.minY, bounds.maxY].every(Number.isFinite) &&
      (bounds.maxX > bounds.minX || bounds.maxY > bounds.minY),
  );
  const problems = columnProblems(motor, handle, layoutId, result);
  check(`${layoutId}: its columns match the contract's presence table`, problems.length === 0);
  for (const problem of problems) process.stdout.write(`#   ${problem}\n`);
  check(
    `${layoutId}: its JSON face carries the same nodes`,
    JSON.parse(motor.toJSON(handle)).nodes.id.length === 2,
  );
}
check("every registered layout ran through the published SDK", ran === registered.length && ran > 0);

// --- POST and ANALYSIS -------------------------------------------------------------------
//
// The same discipline as the layout loop above: the capability and analysis lists come
// from the module's own registries (`Motor#posts` / `Motor#analyses`, i.e.
// `gm_post_count`/`gm_post_id` and `gm_analysis_count`/`gm_analysis_id`), never from names
// written here. A POST capability or an analysis registered after this script was written
// is therefore covered with no edit to this file — which is the only way these two loops
// can keep meaning "every one" as the registries grow.

// The fixture the POST and ANALYSIS loops run over: a five-node graph with a parallel
// edge, a self-loop and a hierarchy chain, so a routing pass meets an enclosed node, a
// style pass meets a loop and a fan, and depth has a tree to count. Built through the
// published `build`, never through a raw export.
const staged = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d", "z"].map((id) => node(id, "record")),
  edges: [
    { id: "r0", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "r1", source: "b", target: "c", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "r2", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "loop", source: "a", target: "a", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
    { id: "h0", source: "b", target: "d", kind: "hierarchy", label: "parent_of", strength: 0.5, directed: true, record_id: null },
  ],
});
const stagedHandle = motor.build(staged);
check("the POST/ANALYSIS fixture builds", motor.nodeCount(stagedHandle) === 5);

const posts = typeof motor.posts === "function" ? motor.posts() : [];
check("the SDK publishes the module's POST registry (C1)", posts.length > 0);
check("the POST registry names the two bundlers, the route and the four styles", (() => {
  const want = [
    "post.bundle.fdeb", "post.bundle.mingle", "post.route.grid",
    "post.style.straight", "post.style.orthogonal", "post.style.quadratic", "post.style.bezier",
  ];
  return want.every((id) => posts.includes(id)) && new Set(posts).size === posts.length;
})());

// The contract's own table, restated the way a consumer reads it: which edge geometry
// kind each named capability emits. A pass that emitted a different kind would be
// inconsistent with the contract even if it ran without refusing.
const POST_EDGE_KIND = new Map([
  ["post.bundle.fdeb", "Polyline"],
  ["post.bundle.mingle", "Polyline"],
  ["post.route.grid", "Polyline"],
  ["post.style.straight", "Line"],
  ["post.style.orthogonal", "Polyline"],
  ["post.style.quadratic", "Curve"],
  ["post.style.bezier", "Curve"],
]);

motor.layout(stagedHandle, "layout.grid");
const gridNodeX = Array.from(motor.column(stagedHandle, ColumnId.NodeX));
let postRan = 0;
for (const postId of posts) {
  motor.layout(stagedHandle, "layout.grid");
  // As with the analyses: a refusal is a failure to report, not a reason to abort the
  // loop, so a broken pass does not hide the state of the other six.
  let result;
  try {
    result = motor.post(stagedHandle, postId);
  } catch (error) {
    check(`${postId}: the published SDK runs the pass`, false);
    process.stdout.write(`#   ${String(error)}\n`);
    continue;
  }
  postRan += 1;
  process.stdout.write(`# ${postId}: ${result.nodeKind} nodes / ${result.edgeKind} edges\n`);
  check(`${postId}: it reports the edge kind the contract names`, result.edgeKind === POST_EDGE_KIND.get(postId));
  check(`${postId}: it leaves the node kind alone`, result.nodeKind === "Point");
  check(`${postId}: the node positions are the layout's, unmoved`, (() => {
    const now = Array.from(motor.column(stagedHandle, ColumnId.NodeX));
    return now.length === gridNodeX.length && now.every((v, i) => Object.is(v, gridNodeX[i]));
  })());
  const problems = columnProblems(motor, stagedHandle, postId, result);
  check(`${postId}: its columns match the contract's presence table`, problems.length === 0);
  for (const problem of problems) process.stdout.write(`#   ${problem}\n`);
  check(
    `${postId}: its JSON face carries the fixture's nodes`,
    JSON.parse(motor.toJSON(stagedHandle)).nodes.id.length === 5,
  );
  check(`${postId}: every edge got a row when its kind stores one`, (() => {
    const offsets = motor.column(stagedHandle, ColumnId.EdgeOffsets);
    if (offsets === null) return result.edgeKind === "Line";
    const points = motor.column(stagedHandle, ColumnId.EdgePts);
    return offsets.length === 6 && points.length === 2 * offsets[offsets.length - 1];
  })());
}
check("every registered POST capability ran through the published SDK", postRan === posts.length && postRan > 0);

// A post pass needs a layout run: refused with a typed error, not a silent no-op, and
// never a drawing over nothing.
{
  const bare = motor.build(staged);
  let refused;
  try {
    motor.post(bare, posts[0]);
  } catch (error) {
    refused = error;
  }
  check("a post pass with no layout run yet is refused", refused instanceof PostRefusedError);
  check("the refusal names NoGeometryYet", refused?.codeName === "NoGeometryYet");
  check("an unknown post id is refused before the ABI is asked to run it", await refusedWith(PostRefusedError, () => motor.post(bare, "post.style.none")));
  check("an unknown post id in the registry listing is not silently accepted", !posts.includes("post.style.none"));
  motor.release(bare);
}

// The analyses, over a graph with **no layout run at all** — the maths is a function of
// the topology, so this is a supported state and not a refusal.
const analyses = typeof motor.analyses === "function" ? motor.analyses() : [];
check("the SDK publishes the module's ANALYSIS registry (C1)", analyses.length > 0);
check("the ANALYSIS registry names the eight registered analyses", analyses.length === 8 && new Set(analyses).size === 8);
let analysed = 0;
for (const analysisId of analyses) {
  // A refusal here is a real failure (a face that does not name itself, a value of the
  // wrong type, a length that disagrees with `nodeCount`), so it is *caught* and reported
  // as `not ok` rather than left to abort the script: a crash would take every check
  // after this loop with it, hiding whatever else is broken.
  let result;
  try {
    result = motor.analysis(stagedHandle, analysisId);
  } catch (error) {
    check(`${analysisId}: the published SDK returns a typed result`, false);
    process.stdout.write(`#   ${String(error)}\n`);
    continue;
  }
  analysed += 1;
  process.stdout.write(`# ${analysisId}: ${result.kind} x ${result.nodeCount} = ${JSON.stringify(result.values)}\n`);
  check(`${analysisId}: it names itself`, result.id === analysisId);
  check(`${analysisId}: one value per node, in order`, result.values.length === 5);
  check(`${analysisId}: its element type is one of the two the ABI names`, result.kind === "f64" || result.kind === "u32");
  check(`${analysisId}: its values are finite`, result.values.every(Number.isFinite));
  if (result.kind === "u32") {
    check(`${analysisId}: a labelling is whole and non-negative`, result.values.every((v) => Number.isInteger(v) && v >= 0));
  }
  // The three escape hatches, each present exactly when the analysis hands one back.
  if (analysisId === "analysis.centrality.eigenvector") {
    check("the eigenvector reports its convergence flag, not just its numbers", typeof result.converged === "boolean");
  } else {
    check(`${analysisId}: carries no convergence flag`, result.converged === undefined);
  }
  if (analysisId === "analysis.communities.louvain") {
    check("Louvain reports the modularity of the partition it returned", Number.isFinite(result.modularity));
  } else {
    check(`${analysisId}: carries no modularity`, result.modularity === undefined);
  }
  if (analysisId === "analysis.depth.bfs") {
    check("depth reports the deepest level it reached", Number.isInteger(result.max) && result.max >= 0);
  } else {
    check(`${analysisId}: carries no depth maximum`, result.max === undefined);
  }
}
check("every registered analysis ran through the published SDK", analysed === analyses.length && analysed > 0);
// The checks below read two named results, so they only run if both were produced.
if (analysed === 0) {
  check("an analysis ran, so its values could be read at all", false);
} else {

// The component labellings, read as the contract states them: a graph with two roots
// (`{a,b,c}` and `{d,z}`) is two components, numbered by the first member seen ascending.
check(
  "the weak components of the fixture are the two roots, numbered by first member",
  (() => {
    const r = motor.analysis(stagedHandle, "analysis.components.weak");
    return r.kind === "u32" && r.values[0] === 0 && r.values[4] === 1;
  })(),
);
check(
  "the hierarchy depth of the fixture counts out from its roots",
  (() => {
    const r = motor.analysis(stagedHandle, "analysis.depth.bfs");
    // `b` is the only node with a parent (it has the `h0` hierarchy edge to `d`), so
    // `a`, `b`, `c` and `z` are all roots — four roots, which hang off the hidden
    // virtual root, putting every one of them at depth 1 and `d` (a child of `b`) at 2.
    return (
      r.kind === "u32" &&
      r.values[0] === 1 && r.values[1] === 1 && r.values[2] === 1 &&
      r.values[3] === 2 && r.values[4] === 1 &&
      r.max === 2
    );
  })(),
);
// The stage results the SDK hands back are its own, not a re-derivation: the same
// handle, the same ids, asked twice, must answer identically — and a pass run over a
// *freshly laid out* handle must match one run over the handle the loop left behind, or
// "idempotent" would be a claim nothing checks.
check("the same post pass twice answers the same edges", (() => {
  const first = motor.layout(stagedHandle, "layout.grid") && motor.post(stagedHandle, "post.style.bezier");
  const a = JSON.stringify(motor.toJSON(stagedHandle));
  motor.layout(stagedHandle, "layout.grid");
  motor.post(stagedHandle, "post.style.bezier");
  const b = JSON.stringify(motor.toJSON(stagedHandle));
  return first !== null && a === b;
})());
check(
  "the degree centrality agrees with the topology it was computed from",
  (() => {
    const r = motor.analysis(stagedHandle, "analysis.centrality.degree");
    // `b` is incident to `r0`, the parallel `r2`, `r1` and the hierarchy edge `h0`; `a`
    // to `r0`, `r2` and its own self-loop, which a degree column counts **twice** (it is
    // both endpoints); `c` only to `r1`; `d` only to `h0`; `z` to nothing.
    return (
      r.kind === "f64" && r.values[0] === 4 && r.values[1] === 4 &&
      r.values[2] === 1 && r.values[3] === 1 && r.values[4] === 0
    );
  })(),
);
check("an unknown analysis id is refused", await refusedWith(AnalysisRefusedError, () => motor.analysis(stagedHandle, "analysis.components.none")));
check("a released handle is refused by both stages (C6)", (() => {
  motor.release(stagedHandle);
  let post = false;
  let analysis2 = false;
  try {
    motor.post(stagedHandle, posts[0]);
  } catch {
    post = true;
  }
  try {
    motor.analysis(stagedHandle, analyses[0]);
  } catch {
    analysis2 = true;
  }
  return post && analysis2;
})());
}



// Everything below reads a run the loop above leaves behind (the last registered layout's),
// so with no layout at all there is nothing to read and every check would be a thrown
// error rather than a verdict. Said once, here, instead of at each of them.
if (ran === 0) {
  check("a layout ran, so the transport could be exercised at all", false);
} else {
  const beforeTamper = motor.toJSON(handle);
  check("toJSON succeeds before any tamper", typeof beforeTamper === "string" && beforeTamper.includes('"a"'));

  // C8: a column view is a real zero-copy alias into the motor's own memory — writing NaN
  // through it and then asking the motor to encode a face is refused (D9), not silently
  // written through to the wire.
  const epochBeforeTamper = motor.epoch;
  const x = motor.column(handle, ColumnId.NodeX);
  x[0] = Number.NaN;
  check("writing through a column view does not itself move the epoch", motor.epoch === epochBeforeTamper);
  let tamperRefused = false;
  try {
    motor.toJSON(handle);
  } catch (error) {
    tamperRefused = error instanceof TamperedGeometryError;
  }
  check("a NaN written through a column view refuses toJSON (D9)", tamperRefused);

  const epochBeforeRelease = motor.epoch;
  motor.release(handle);
  check("release moves the epoch forward", motor.epoch > epochBeforeRelease);
  let releasedRefuses = false;
  try {
    motor.nodeCount(handle);
  } catch {
    releasedRefuses = true;
  }
  check("a released handle is refused, not silently answered (C6)", releasedRefuses);
}

// Regression (review finding, MAJOR): createMotor() must never throw on a load failure
// (prompt.md §3.2, phase-04-wasm-sdk.md step 5: "warn-and-degrade rather than throw ...
// A motor that throws on load takes the host page down with it") — it must resolve to a
// degraded Motor whose own calls fail predictably instead, never fabricated data.
{
  const priorKillSwitch = globalThis.__GM_DISABLE_WASM__;
  globalThis.__GM_DISABLE_WASM__ = true;
  let threw = false;
  let degraded;
  try {
    degraded = await createMotor(bytes);
  } catch {
    threw = true;
  }
  check("createMotor_never_throws_on_kill_switch", !threw);
  let buildRefused = false;
  try {
    degraded?.build(ingest);
  } catch (error) {
    buildRefused = error instanceof WasmUnavailableError;
  }
  check("degraded_motor_build_fails_predictably_kill_switch", buildRefused);
  // A degraded motor must not answer `layouts()` with an empty registry: "this module has
  // no layouts" and "this module never loaded" are different facts, and only one of them
  // is true. It refuses like every other method that needs the module.
  check(
    "degraded_motor_layouts_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.layouts()),
  );
  // Same for the two registries added after it: an empty list would read as "this module
  // has no post capabilities" / "no analyses", which is a different fact from "this
  // module never loaded", and only one of them is true.
  check(
    "degraded_motor_posts_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.posts()),
  );
  check(
    "degraded_motor_analyses_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.analyses()),
  );
  check("degraded_motor_post_fails_predictably_kill_switch", await refusedWith(WasmUnavailableError, () => degraded?.post(1, "post.style.straight")));
  check("degraded_motor_analysis_fails_predictably_kill_switch", await refusedWith(WasmUnavailableError, () => degraded?.analysis(1, "analysis.components.weak")));
  globalThis.__GM_DISABLE_WASM__ = priorKillSwitch;
}
{
  resetForTests();
  let threw = false;
  let degraded;
  try {
    degraded = await createMotor(new Uint8Array([0, 1, 2, 3]));
  } catch {
    threw = true;
  }
  check("createMotor_never_throws_on_compile_failure", !threw);
  let buildRefused = false;
  try {
    degraded?.build(ingest);
  } catch (error) {
    buildRefused = error instanceof WasmUnavailableError;
  }
  check("degraded_motor_build_fails_predictably_compile_failure", buildRefused);
  resetForTests();
}

process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
process.exit(failures === 0 ? 0 : 1);
