/**
 * The end-to-end check the unit tests cannot be: the REAL wasm module, the REAL
 * SDK, every layout the module registers, and the studio's own column → draw
 * list mapping over each result. The POST and ANALYSIS halves live in
 * `checkStages.ts` — same module, same run, split because they are a different
 * question about a different part of the ABI.
 *
 * It is a script rather than a `*.test.ts` because it needs
 * `app/public/graph_wasm.wasm` — a build artefact. `scripts/studio.sh check`
 * stages that first and then runs this, so the check is never silently skipped:
 * if the wasm is missing, this fails loudly instead of passing quietly.
 *
 * Run directly with:
 *   node --experimental-strip-types --experimental-loader /w/tests/ts-extension-loader.mjs scripts/verify-motor.ts
 *
 * (The loader is the engine's own resolution shim for its extensionless relative
 * imports; the studio's own unit tests do not need it, and deliberately do not
 * use it — see src/core/fit.ts.)
 */

import { readFile } from "node:fs/promises";

import { createMotor, type Handle, type Motor } from "../../crates/graph-sdk-js/src/index.ts";
import { type ColumnInput, buildDrawList, describeColumns } from "../src/core/drawList.ts";
import { normaliseIngest } from "../src/core/ingestText.ts";
import { syntheticIngest } from "../src/core/synthetic.ts";
import { edgeKinds, stylesFor } from "../src/render/palette.ts";
import { checkAnalyses, checkList, checkPostComposition, checkPosts, checkRampEnds, readColumns } from "./checkStages.ts";

const failures: string[] = [];

function check(condition: boolean, what: string): void {
  if (!condition) failures.push(what);
}

function ms(value: number): string {
  return `${value.toFixed(2)}ms`;
}

/** One build + one run over `handle`, printed as a row. */
function runOnce(motor: Motor, handle: Handle, layoutId: string): string {
  const started = performance.now();
  const result = motor.layout(handle, layoutId);
  const elapsed = performance.now() - started;
  const columns: ColumnInput = readColumns(motor, handle, result.nodeKind, result.edgeKind);
  const list = buildDrawList(columns);
  checkList(failures, layoutId, list, result.nodeCount);
  const present = describeColumns(columns).filter((row) => row.length !== null).length;
  return [
    layoutId.padEnd(26),
    `${result.nodeKind}/${result.edgeKind}`.padEnd(16),
    `${list.nodes.length} nodes`.padEnd(12),
    `${list.edges.length} edges`.padEnd(12),
    `${present} columns`.padEnd(12),
    ms(elapsed),
  ].join(" ");
}

/** Run a layout, turning a refusal into a recorded failure rather than a crash —
 *  a refused fixture must still leave the rest of the check running. */
function runOrRecord(motor: Motor, handle: Handle, layoutId: string, label: string): void {
  try {
    console.log(`    ${runOnce(motor, handle, layoutId)}`);
  } catch (error) {
    const why = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
    failures.push(`${label} on ${layoutId}: ${why}`);
    console.log(`    ${layoutId} REFUSED — ${why}`);
  }
}

/** Every layout the module registers, over one synthetic graph. */
function checkLayouts(motor: Motor, handle: Handle, layouts: readonly string[]): void {
  console.log(`\n${layouts.length} layout(s) over the synthetic graph:\n`);
  for (const id of layouts) console.log(`  ${runOnce(motor, handle, id)}`);
}

/** Three fixtures, each on a layout that suits its shape. */
async function checkFixtures(motor: Motor, layouts: ReadonlySet<string>): Promise<void> {
  const wanted: readonly (readonly [string, string])[] = [
    ["fixtures/dag/wide-layer.json", "layout.dag.sugiyama"],
    ["fixtures/force/tree.json", "layout.force.barnes_hut"],
    ["fixtures/hierarchy/forest.json", "layout.tree.tidy"],
  ];
  console.log("");
  for (const [name, layoutId] of wanted) {
    check(layouts.has(layoutId), `${name}: this build does not register ${layoutId}`);
    if (!layouts.has(layoutId)) continue;
    const text = await readFile(new URL(`../public/${name}`, import.meta.url), "utf8");
    const result = normaliseIngest(text, name);
    console.log(`  ${name} → ${result.doc.nodes.length} nodes / ${result.doc.edges.length} edges, ${result.notes.length} note(s)`);
    const handle = motor.build(result.json);
    runOrRecord(motor, handle, layoutId, name);
    motor.release(handle);
  }
}

async function main(): Promise<void> {
  const bytes = await readFile(new URL("../public/graph_wasm.wasm", import.meta.url));
  const motor = await createMotor(bytes);
  check(motor.available, "the motor is degraded — the wasm module did not load");
  if (!motor.available) {
    console.error("the motor did not load; nothing below is meaningful");
    process.exit(1);
  }

  const layouts = motor.layouts();
  const posts = motor.posts();
  const analyses = motor.analyses();
  console.log(`layouts registered by the module: ${layouts.length}`);
  for (const id of layouts) console.log(`  ${id}`);
  check(layouts.length > 0, "the module registered no layouts");
  check(posts.length > 0, "the module registered no POST capabilities");
  check(analyses.length > 0, "the module registered no analyses");

  const ingest = syntheticIngest({ seed: 1, nodeCount: 120, degree: 3 });
  const doc = normaliseIngest(ingest, "synthetic").doc;
  check(stylesFor(doc).length === doc.nodes.length, "stylesFor disagrees with the document's node count");
  check(edgeKinds(doc).length === doc.edges.length, "edgeKinds disagrees with the document's edge count");
  console.log(`\nsynthetic graph: ${doc.nodes.length} nodes, ${doc.edges.length} edges`);

  const buildStart = performance.now();
  const handle = motor.build(ingest);
  console.log(`build: ${ms(performance.now() - buildStart)}`);
  checkLayouts(motor, handle, layouts);
  await checkFixtures(motor, new Set(layouts));
  checkPosts(motor, handle, posts, failures);
  checkPostComposition(motor, handle, posts, failures);
  motor.release(handle);

  // Analyses run against a handle that has run NO layout, which is the ABI's own
  // claim: every analysis is a function of the topology.
  const analysisHandle = motor.build(ingest);
  checkAnalyses(motor, analysisHandle, analyses, failures);
  const centrality = motor.analysis(analysisHandle, "analysis.centrality.betweenness");
  if (centrality.kind === "f64") checkRampEnds(centrality, failures);
  motor.release(analysisHandle);

  if (failures.length > 0) {
    console.error(`\n${failures.length} FAILURE(S):`);
    for (const failure of failures) console.error(`  - ${failure}`);
    process.exit(1);
  }
  console.log("\nall layouts, post passes, analyses and fixtures mapped cleanly");
}

await main();
