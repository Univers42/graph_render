/**
 * Motor units → studio world units. The motor lays graphs out in its own units
 * (a grid cell is 1, a tidy-tree level is 1), while the renderer's constants —
 * the Point radius, label size, edge width — are pixel-scale. Drawn raw, a
 * 120-node grid is 11 units wide, every node overlaps its neighbours, and
 * fit-to-view hits MAX_ZOOM long before the graph fills the panel.
 *
 * So one uniform factor per run is applied to every coordinate and size,
 * which keeps the geometry exactly similar (a Box stays the same shape relative
 * to its neighbours) while putting the typical node spacing at TARGET_SPACING.
 */

import type { DrawList, EdgeDraw, NodeDraw } from "./drawList.ts";

/** Typical centre-to-centre distance after scaling, in world px. */
export const TARGET_SPACING = 56;

/**
 * Ponytail: "typical spacing" is sqrt(bounding-box area / n) — the spacing of a
 * uniform spread. A layout with one dense clump and a few far outliers reads as
 * sparse and gets under-scaled (the clump overlaps); a collinear layout falls
 * back to extent / n. Zooming in is the escape hatch.
 */
export function typicalSpacing(nodes: readonly NodeDraw[]): number {
  if (nodes.length < 2) return 0;
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const node of nodes) {
    minX = Math.min(minX, node.x);
    minY = Math.min(minY, node.y);
    maxX = Math.max(maxX, node.x);
    maxY = Math.max(maxY, node.y);
  }
  const width = maxX - minX;
  const height = maxY - minY;
  const area = width * height;
  if (area > 0) return Math.sqrt(area / nodes.length);
  return Math.max(width, height) / (nodes.length - 1);
}

/** The factor that brings `nodes` to TARGET_SPACING; 1 when there is no spread. */
export function worldScaleFor(nodes: readonly NodeDraw[]): number {
  const spacing = typicalSpacing(nodes);
  return spacing > 0 && Number.isFinite(spacing) ? TARGET_SPACING / spacing : 1;
}

function scaleEdge(edge: EdgeDraw, factor: number): EdgeDraw {
  if (edge.pts.length === 0) return edge;
  const pts = new Float32Array(edge.pts.length);
  for (let i = 0; i < pts.length; i += 1) pts[i] = edge.pts[i] * factor;
  return { ...edge, pts };
}

/** `list` with every coordinate and size multiplied by `factor`. */
export function scaleDrawList(list: DrawList, factor: number): DrawList {
  if (factor === 1) return list;
  return {
    ...list,
    nodes: list.nodes.map((node) => ({
      ...node,
      x: node.x * factor,
      y: node.y * factor,
      w: node.w * factor,
      h: node.h * factor,
      r: list.nodeKind === "Point" ? node.r : node.r * factor,
    })),
    edges: list.edges.map((edge) => scaleEdge(edge, factor)),
  };
}

/** Normalise one run to the studio's world scale. */
export function toStudioWorld(list: DrawList): DrawList {
  return scaleDrawList(list, worldScaleFor(list.nodes));
}
