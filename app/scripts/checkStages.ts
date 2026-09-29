/**
 * The stage half of the live motor check: POST passes and analyses, through the
 * real wasm module and the studio's own projections.
 *
 * Split out of `verify-motor.ts` because the two halves answer different
 * questions — this one is about the CONTRACT of the two stage surfaces (a pass
 * replaces edge geometry and never changes the edge or node count; two passes
 * compose as the second alone; a face covers every node exactly once and a `u32`
 * face is integral), the other about the layout half of the registry.
 *
 * A script, not a suite entry, for the same reason: it needs `graph_wasm.wasm`.
 */

import { type AnalysisResult, type Handle, type Motor } from "../../crates/graph-sdk-js/src/index.ts";
import { type ColumnInput, type DrawList, buildDrawList, describeColumns } from "../src/core/drawList.ts";
import { analysisLegend, describeAnalysis, fillsFor, normalise } from "../src/core/analysis.ts";
import { RAMP_STOPS } from "../src/render/analysisColours.ts";
import { type RunReport, layoutReport, withPost } from "../src/motor/report.ts";

/** The layout the pass checks are built on: a straight Line geometry is what
 *  makes a pass CHANGING the kind observable rather than assumed. */
export const POST_LAYOUT = "layout.force.barnes_hut";

/** Everything one stage check found wrong, appended to the caller's list rather
 *  than thrown: a refused pass must still leave the rest of the check running. */
export type Failures = string[];

function ms(value: number): string {
  return `${value.toFixed(2)}ms`;
}

/** Every column this run can carry, read in one block: the next motor call
 *  invalidates them all (C7). */
export function readColumns(
  motor: Motor,
  handle: Handle,
  nodeKind: ColumnInput["nodeKind"],
  edgeKind: ColumnInput["edgeKind"],
): ColumnInput {
  const read = (id: number): Float32Array | Uint32Array | null => motor.column(handle, id as never);
  return {
    nodeKind, edgeKind,
    x: read(0) as Float32Array | null, y: read(1) as Float32Array | null,
    r: read(2) as Float32Array | null, w: read(3) as Float32Array | null, h: read(4) as Float32Array | null,
    source: read(5) as Uint32Array | null, target: read(6) as Uint32Array | null,
    offsets: read(9) as Uint32Array | null, pts: read(10) as Float32Array | null,
    curveDegree: read(11) as Uint32Array | null,
  };
}

/** The per-kind invariants the renderer depends on, checked on real output. */
export function checkList(failures: Failures, label: string, list: DrawList, nodeCount: number): void {
  const bad = (condition: boolean, what: string): void => {
    if (!condition) failures.push(`${label}: ${what}`);
  };
  bad(list.nodes.length === nodeCount, `draw list has ${list.nodes.length} of ${nodeCount} nodes`);
  bad(list.edges.every((edge) => edge.pts.length % 2 === 0), "an edge path has an odd coordinate count");
  for (const node of list.nodes) {
    bad(Number.isFinite(node.x) && Number.isFinite(node.y), `node ${node.index} is not finite`);
    if (list.nodeKind === "Box") bad(node.w > 0 && node.h > 0, `Box node ${node.index} has no extent`);
    if (list.nodeKind === "Circle") bad(node.r >= 0, `Circle node ${node.index} has r=${node.r}`);
  }
}

/**
 * The CSR shape of every `pts` column, which is the whole of what the draw list
 * slices: `offsets` is one entry per edge plus one, from 0 and never decreasing,
 * and `pts` is exactly two coordinates per point. A run whose `pts` runs past
 * `offsets` is what `DrawListError` exists to refuse, and this is where that
 * refusal would first be observed on real output.
 */
function checkPathShape(failures: Failures, label: string, columns: ColumnInput, edgeCount: number): void {
  if (columns.edgeKind === "Line") return;
  const offsets = columns.offsets;
  const pts = columns.pts;
  if (offsets === null || pts === null) {
    failures.push(`${label}: a ${columns.edgeKind} run has no offsets/pts columns`);
    return;
  }
  const bad = (condition: boolean, what: string): void => {
    if (!condition) failures.push(`${label}: ${what}`);
  };
  bad(offsets.length === edgeCount + 1, `offsets has ${offsets.length} entries for ${edgeCount} edges`);
  bad(offsets[0] === 0, "offsets does not start at 0");
  bad(offsets[edgeCount] * 2 === pts.length, `offsets end (${offsets[edgeCount]}) disagrees with ${pts.length} coordinates`);
  for (let i = 1; i < offsets.length; i += 1) bad(offsets[i] >= offsets[i - 1], `offsets decreases at ${i}`);
  for (let i = 0; i < pts.length; i += 1) bad(Number.isFinite(pts[i]), `coordinate ${i} is not finite`);
}

/** The declared kind and the columns actually carried must agree: a `Polyline`
 *  with no `pts` would be drawn as a straight line, and a `Line` carrying points
 *  would have them silently ignored. */
function checkKindAgainstColumns(failures: Failures, label: string, report: RunReport): void {
  const ptsLength = report.columns.find((row) => row.name === "edge.pts")?.length ?? null;
  if (report.edgeKind === "Line") {
    if (ptsLength !== null) failures.push(`${label}: a Line run carries an edge.pts column (${ptsLength})`);
    return;
  }
  if (ptsLength === null) failures.push(`${label}: a ${report.edgeKind} run has no edge.pts column`);
}

/** One layout, then one pass, then the columns — in that order, because every
 *  motor call invalidates the views the last one handed out (C7). */
function layoutThenPost(motor: Motor, handle: Handle, postId: string): RunReport {
  const run = motor.layout(handle, POST_LAYOUT);
  const base = layoutReport(POST_LAYOUT, run, { buildMs: 0, layoutMs: 0 }, readColumns(motor, handle, run.nodeKind, run.edgeKind));
  const result = motor.post(handle, postId);
  return withPost(base, result, readColumns(motor, handle, result.nodeKind, result.edgeKind), 0);
}

/**
 * Every registered POST pass, each over a FRESH layout, through the studio's own
 * `layoutReport`/`withPost` so the report the panel would show is the one checked.
 * Fresh because a pass reads the LAYOUT's edges and never the previous pass's
 * (docs/contract/wasm-abi.md "POST") — running them in sequence would hide a pass
 * that read its predecessor.
 */
export function checkPosts(motor: Motor, handle: Handle, posts: readonly string[], failures: Failures): void {
  console.log(`\n${posts.length} POST pass(es) over ${POST_LAYOUT}:\n`);
  for (const id of posts) {
    try {
      const run = motor.layout(handle, POST_LAYOUT);
      const base = layoutReport(POST_LAYOUT, run, { buildMs: 0, layoutMs: 0 }, readColumns(motor, handle, run.nodeKind, run.edgeKind));
      const started = performance.now();
      const result = motor.post(handle, id);
      const elapsed = performance.now() - started;
      const columns = readColumns(motor, handle, result.nodeKind, result.edgeKind);
      const report = withPost(base, result, columns, elapsed);
      checkList(failures, id, report.list, report.nodeCount);
      checkPathShape(failures, id, columns, report.list.edges.length);
      checkKindAgainstColumns(failures, id, report);
      checkAgainstBase(failures, id, base, report);
      console.log(
        `  ${id.padEnd(28)} ${`${report.nodeKind}/${report.edgeKind}`.padEnd(16)}` +
        `${`${report.list.edges.length} edges`.padEnd(12)}${ms(elapsed)}`,
      );
    } catch (error) {
      const why = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
      failures.push(`post ${id}: ${why}`);
      console.log(`  ${id} REFUSED — ${why}`);
    }
  }
}

/** What a pass must NOT change: the node geometry and the node count (a pass
 *  redraws edges over positions it did not move) and the EDGE COUNT (it redraws
 *  edges, it does not add or drop them). The edge KIND it may change, and must
 *  restate. */
function checkAgainstBase(failures: Failures, id: string, base: RunReport, report: RunReport): void {
  const bad = (condition: boolean, what: string): void => {
    if (!condition) failures.push(`${id}: ${what}`);
  };
  bad(report.postId === id, `the report names ${String(report.postId)}`);
  bad(report.nodeCount === base.nodeCount, `the node count changed (${base.nodeCount} → ${report.nodeCount})`);
  bad(report.nodeKind === base.nodeKind, `the node geometry kind changed (${base.nodeKind} → ${report.nodeKind})`);
  bad(report.edgeCount === base.edgeCount, `the edge count changed (${base.edgeCount} → ${report.edgeCount})`);
  bad(report.nodeCount === report.list.nodes.length, "the pass's node count is not the draw list's");
  bad(report.nodeKind === report.list.nodeKind, "the pass's node kind is not the draw list's");
  bad(report.edgeKind === report.list.edgeKind, "the pass's edge kind is not the draw list's");
  bad(report.edgeCount === report.list.edges.length, "the edge count is not the draw list's");
  bad(report.layoutId === base.layoutId, `the layout id changed to ${report.layoutId}`);
  bad(report.buildMs === base.buildMs && report.layoutMs === base.layoutMs, "the build/layout durations changed");
}

/**
 * Two passes in a row must give the same picture as the second alone, which is
 * the ABI's claim that a pass reads the LAYOUT's edges. Checked on the first two
 * registered passes, through the studio's own report builder.
 */
export function checkPostComposition(motor: Motor, handle: Handle, posts: readonly string[], failures: Failures): void {
  const [first, second] = posts;
  if (first === undefined || second === undefined) return;
  try {
    const alone = layoutThenPost(motor, handle, second);
    motor.layout(handle, POST_LAYOUT);
    motor.post(handle, first);
    const chained = layoutThenPost(motor, handle, second);
    if (JSON.stringify(alone.list) !== JSON.stringify(chained.list)) {
      failures.push(`${first} then ${second} differs from ${second} alone — a pass read the previous pass's edges`);
    }
    if (alone.list.edgeKind !== chained.list.edgeKind) {
      failures.push(`${first} then ${second}: the edge kinds differ from ${second} alone`);
    }
    console.log(`\ncomposition: ${first} then ${second} == ${second} alone (${alone.list.edgeKind})`);
  } catch (error) {
    const why = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
    failures.push(`post composition ${first}+${second}: ${why}`);
    console.log(`composition REFUSED — ${why}`);
  }
}

/** One analysis's face, checked against the graph it was run over, plus the
 *  studio's own projection over it. */
function checkFace(failures: Failures, id: string, face: AnalysisResult, nodeCount: number): void {
  const bad = (condition: boolean, what: string): void => {
    if (!condition) failures.push(`${id}: ${what}`);
  };
  bad(face.id === id, `the motor answered for ${face.id}`);
  bad(face.nodeCount === nodeCount, `covers ${face.nodeCount} of ${nodeCount} nodes`);
  bad(face.values.length === nodeCount, `${face.values.length} values for ${nodeCount} nodes`);
  bad(face.values.every((v) => Number.isFinite(v)), "a value is not finite");
  if (face.kind === "u32") {
    bad(face.values.every((v) => Number.isInteger(v) && v >= 0), "a u32 value is not a non-negative integer");
  }
  const fills = fillsFor(face);
  bad(fills.length === nodeCount, `${fills.length} fills for ${nodeCount} nodes`);
  bad(fills.every((fill) => /^#[0-9a-f]{6}$/.test(fill)), "a fill is not a #rrggbb colour");
  bad(analysisLegend(face).length > 0, "the legend is empty");
  for (const row of describeAnalysis(face)) {
    bad(!row.value.includes("NaN") && !row.value.includes("undefined"), `the readout says ${row.label}=${row.value}`);
  }
}

/** Every registered analysis, over a handle that has run NO layout, with the
 *  studio's own projection over each face. Analyses are functions of the topology
 *  (docs/contract/wasm-abi.md "ANALYSIS"), so a build alone must answer every id. */
export function checkAnalyses(motor: Motor, handle: Handle, analyses: readonly string[], failures: Failures): void {
  console.log(`\n${analyses.length} analysis(es) over the synthetic graph:\n`);
  const nodeCount = motor.nodeCount(handle);
  for (const id of analyses) {
    try {
      const face = motor.analysis(handle, id);
      checkFace(failures, id, face, nodeCount);
      const extras = [
        face.converged === undefined ? null : `converged=${String(face.converged)}`,
        face.modularity === undefined ? null : `modularity=${face.modularity.toFixed(6)}`,
        face.max === undefined ? null : `max=${face.max}`,
      ].filter((row): row is string => row !== null);
      console.log(
        `  ${id.padEnd(38)} ${face.kind.padEnd(5)}${`${face.nodeCount} values`.padEnd(14)}` +
        `${extras.length === 0 ? "no scalar extras" : extras.join(" ")}`,
      );
    } catch (error) {
      const why = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
      failures.push(`analysis ${id}: ${why}`);
      console.log(`  ${id} REFUSED — ${why}`);
    }
  }
}

/**
 * The ramp the overlay paints with, against a real face: both of its extremes must
 * be ON a node, or the legend names a colour nothing wears. The interior stops are
 * deliberately not required — a betweenness face is heavily skewed, so a mid stop
 * can legitimately be absent, and the studio does not claim otherwise.
 */
export function checkRampEnds(face: AnalysisResult, failures: Failures): void {
  const fills = fillsFor(face);
  const lo = Math.min(...face.values);
  const hi = Math.max(...face.values);
  if (normalise(lo, lo, hi) !== 0) failures.push(`${face.id}: normalise at the minimum is not 0`);
  if (normalise(hi, lo, hi) !== 1) failures.push(`${face.id}: normalise at the maximum is not 1`);
  if (hi <= lo) {
    if (new Set(fills).size !== 1) failures.push(`${face.id}: a flat face paints more than one colour`);
    return;
  }
  if (!fills.includes(RAMP_STOPS[0])) failures.push(`${face.id}: the cold end ${RAMP_STOPS[0]} is on no node`);
  if (!fills.includes(RAMP_STOPS[RAMP_STOPS.length - 1])) failures.push(`${face.id}: the hot end is on no node`);
  if (new Set(fills).size === 1) failures.push(`${face.id}: a face with a range paints one colour`);
}
