// A handle's two geometry tags, read back from the module and named.

import { toU32, type RawExports } from "./wasm.ts";
import { RunRefusedError } from "./errors.ts";
import { invoke } from "./calls.ts";
import type { EdgeGeometryKind, Handle, NodeGeometryKind } from "./types.ts";

const NODE_KIND_BY_TAG: readonly NodeGeometryKind[] = ["Point", "Circle", "Box"];
const EDGE_KIND_BY_TAG: readonly EdgeGeometryKind[] = ["Line", "Polyline", "Curve"];

/** A handle's two geometry tags, as this SDK records them: what decides whether a
 *  column is present (C3), and what a run or a post pass reports back. */
export interface GeometryKinds {
  nodeKind: NodeGeometryKind;
  edgeKind: EdgeGeometryKind;
}

/** The two geometry tags, read back so {@link Motor.column} can decide presence from
 *  them (C3). Shared by `layout` and `post` because a POST pass changes the *edge* kind —
 *  routing over a `Line` layout yields a `Polyline`, and a style declares its own — so
 *  the cached kinds have to be re-read after a pass or every column read would be
 *  decided against the pre-pass kind. */
export function readKinds(exports: RawExports, handle: Handle, what: string): GeometryKinds {
  const nodeTag = invoke("gm_geometry_kind", () => exports.gm_geometry_kind(toU32(handle)));
  const edgeTag = invoke("gm_edge_geometry_kind", () => exports.gm_edge_geometry_kind(toU32(handle)));
  const nodeKind = NODE_KIND_BY_TAG[nodeTag];
  const edgeKind = EDGE_KIND_BY_TAG[edgeTag];
  if (nodeKind === undefined || edgeKind === undefined) {
    throw new RunRefusedError(`${what}: unknown geometry tag (node ${nodeTag}, edge ${edgeTag})`);
  }
  return { nodeKind, edgeKind };
}
