/**
 * The local graph as a layer over the style: a 0/1 set of nodes that stay visible, and the
 * bounds a fit uses while it holds. The set is a mask and never a second adjacency; the walk
 * itself is `bfsMask`.
 */
import type { WalkKinds } from "./adjacency.ts";
import type { Bounds } from "./camera.ts";
import type { Style } from "./style.ts";

export interface LocalOptions extends WalkKinds {
  /** 1..5 hops. */
  readonly depth: number;
}

/** What the view keeps while a local graph is shown. */
export interface LocalLayer {
  /** 1 per node that stays visible, or null when the whole graph is shown. */
  visible: Uint8Array | null;
  /** The style the host set, before the layer hid anything. */
  base: Style | null;
  bounds: Bounds | null;
}

export function newLocalLayer(): LocalLayer {
  return { visible: null, base: null, bounds: null };
}

/** Node indices set in `mask`, ascending. */
export function idsOf(mask: Uint8Array): number[] {
  const ids: number[] = [];
  for (let i = 0; i < mask.length; i += 1) if (mask[i] === 1) ids.push(i);
  return ids;
}

/** `style` with every node outside `visible` hidden; a node the style already hid stays hidden. */
export function withLocalHidden(style: Style, visible: Uint8Array | null): Style {
  if (visible === null || visible.length !== style.nodeCount) return style;
  const hidden = new Uint8Array(visible.length);
  for (let i = 0; i < hidden.length; i += 1) hidden[i] = visible[i] === 1 && style.hidden?.[i] !== 1 ? 0 : 1;
  return { ...style, hidden };
}

/** The box of the visible nodes, each grown by its own radius; null when none is visible. */
export function boundsOfVisible(
  at: { readonly x: Float32Array; readonly y: Float32Array },
  visible: Uint8Array,
  radius: Float32Array,
): Bounds | null {
  let box: Bounds | null = null;
  for (let i = 0; i < visible.length; i += 1) {
    if (visible[i] !== 1) continue;
    const grow = radius[i] ?? 0;
    const x = at.x[i] ?? 0;
    const y = at.y[i] ?? 0;
    box = box === null
      ? { minX: x - grow, minY: y - grow, maxX: x + grow, maxY: y + grow }
      : {
        minX: Math.min(box.minX, x - grow), minY: Math.min(box.minY, y - grow),
        maxX: Math.max(box.maxX, x + grow), maxY: Math.max(box.maxY, y + grow),
      };
  }
  return box;
}
