/**
 * Per-node presentation, derived from the ingest document rather than invented
 * here: the engine's own `nodeFill` (records coloured by database, notes and tag
 * hubs on reserved hues) and its `shapeOf` (disc / ring / note, so kind is
 * legible from silhouette alone). The studio adds nothing to that vocabulary — a
 * colour or a shape it made up would be a second, disagreeing one.
 */

import { nodeFill } from "../../../src/core/theme/colors.ts";
import { shapeOf, type NodeShape } from "../../../src/core/render/nodeShape.ts";
import type { EdgeKind, NodeKind } from "../../../src/core/types.ts";
import type { IngestDoc } from "../core/ingestText.ts";

/** What one node looks like, indexed by DENSE index — which is ingest order, the
 *  order the motor's columns are in (`docs/contract/wasm-abi.md` "Column order"). */
export interface NodeStyle {
  readonly fill: string;
  readonly shape: NodeShape;
  readonly kind: NodeKind;
  readonly label: string;
  readonly group: string | null;
}

/** One style per ingest node, in ingest order. A `Box`/`Circle`/`Point` run of a
 *  different node count than the document would be a motor bug; the styles array
 *  is simply shorter and the renderer falls back to a neutral fill. */
export function stylesFor(doc: IngestDoc): NodeStyle[] {
  return doc.nodes.map((node) => ({
    fill: nodeFill({ kind: node.kind, label: node.label, databaseId: node.database_id, source: node.source }),
    shape: shapeOf(node.kind),
    kind: node.kind,
    label: node.label,
    group: node.group,
  }));
}

/** The neutral fill for a dense index with no style: the theme's own ink at low
 *  alpha, so a mismatch is visible as grey rather than as a wrong colour. */
export const NEUTRAL_FILL = "rgba(237, 234, 227, 0.55)";

/** Edge stroke by ingest kind, through the engine's own table. */
export function edgeKinds(doc: IngestDoc): EdgeKind[] {
  return doc.edges.map((edge) => edge.kind);
}
