/**
 * The studio's copy of one run, and what a POST pass does to it.
 *
 * `RunReport` is plain data — no wasm view, no pointer — so it survives the next
 * motor call. It lives here rather than in `session.ts` because the transform a
 * pass applies is pure, which is what makes it testable without a module.
 *
 * **A pass replaces the edge geometry, not the nodes.** Every column of the same
 * handle is re-read after the pass, INCLUDING the geometry kinds: `post.route.grid`
 * over a `Line` layout yields a `Polyline`, and a report that kept the pre-pass
 * kind would draw a routed arc through a painter told there were no points.
 */

import type { EdgeGeometryKind, NodeGeometryKind, PostResult } from "../../../crates/graph-sdk-js/src/index.ts";
import { type ColumnInput, type DrawList, buildDrawList, describeColumns } from "../core/drawList.ts";

/** What one run produced, plus how long each call into the motor took. The two
 *  durations are wall-clock measurements of the HOST around `build` and `layout`
 *  (D8 keeps the clock out of the motor); a pass adds its own. */
export interface RunReport {
  readonly layoutId: string;
  /** The POST capability applied on top of the layout, or `null` when the report
   *  is the layout's own geometry. A cleared pass is a fresh layout run, so it
   *  also reads `null` — see `useStages.ts` for why there is nothing else to say. */
  readonly postId: string | null;
  readonly postMs: number;
  readonly nodeKind: NodeGeometryKind;
  readonly edgeKind: EdgeGeometryKind;
  readonly nodeCount: number;
  readonly edgeCount: number;
  readonly buildMs: number;
  readonly layoutMs: number;
  readonly columns: readonly { name: string; length: number | null }[];
  readonly list: DrawList;
}

/** The report a layout run alone produces: the geometry the layout emitted, with
 *  no pass on top, no pass duration, and the edge count the draw list found. */
export function layoutReport(
  layoutId: string,
  result: { nodeKind: NodeGeometryKind; edgeKind: EdgeGeometryKind; nodeCount: number },
  timings: { buildMs: number; layoutMs: number },
  columns: ColumnInput,
): RunReport {
  const list = buildDrawList(columns);
  return {
    layoutId, postId: null, postMs: 0,
    nodeKind: result.nodeKind, edgeKind: result.edgeKind, nodeCount: result.nodeCount,
    edgeCount: list.edges.length,
    buildMs: timings.buildMs, layoutMs: timings.layoutMs,
    columns: describeColumns(columns), list,
  };
}

/**
 * The report a POST pass produces: the layout's identity and its two durations
 * kept, the pass's own id and duration taken, and every geometry member re-read
 * from the handle AFTER the pass. `columns` is the post-pass column set, so a
 * pass that stops being a `Line` run is described as one, and the draw list is
 * rebuilt from it rather than patched.
 */
export function withPost(
  base: RunReport,
  result: PostResult,
  columns: ColumnInput,
  postMs: number,
): RunReport {
  const list = buildDrawList(columns);
  return {
    layoutId: base.layoutId,
    postId: result.id,
    postMs,
    nodeKind: result.nodeKind,
    edgeKind: result.edgeKind,
    nodeCount: result.nodeCount,
    edgeCount: list.edges.length,
    buildMs: base.buildMs,
    layoutMs: base.layoutMs,
    columns: describeColumns(columns),
    list,
  };
}
