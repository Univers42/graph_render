/**
 * The card beside the node under the pointer: the host's preview when it gave one, else what
 * the document says about the node (verdict 11; new with the host API, none existed before).
 *
 * WHY `aria-hidden`: it follows a pointer a screen reader does not have. Everything it says is
 * in the inspector too, which the keyboard reaches.
 * Caveat: placed where the node was when the pointer reached it. A pan, a zoom or a live settle
 * that moves the node while the card is up leaves the card behind until the pointer moves.
 */
import { memo, useEffect, useState, useSyncExternalStore, type ReactElement } from "react";

import type { View } from "../../../graph-render/src/view.ts";
import type { NodePreview } from "../host/contract.ts";
import type { Previews } from "../host/previews.ts";
import type { GraphMeta } from "../source/meta.ts";
import { PreviewBody } from "./Preview.tsx";

/** Screen pixels between the node's centre and the card's corner, so the card never hides it. */
const OFFSET = 14;

export interface HoverCardProps {
  readonly previews: Pick<Previews, "subscribe" | "shown">;
  readonly meta: GraphMeta | null;
  readonly view: Pick<View, "on" | "camera" | "position">;
}

/** The host's preview, or one made of the node's own label, kind and path. */
export function previewOrOwn(meta: GraphMeta, node: number, preview: NodePreview | null): NodePreview {
  if (preview !== null) return preview;
  const path = meta.paths[node] ?? "";
  const own = { title: meta.labels[node] ?? "", icon: meta.kinds[node] ?? "" };
  return path === "" ? own : { ...own, text: path };
}

/**
 * WHY the hovered node is kept beside the meta it was hovered in: a node index means nothing in
 * the next graph, and a card for it would name a node the pointer is not over.
 */
function useHovered(view: HoverCardProps["view"], meta: GraphMeta | null): number {
  const [hovered, hover] = useState<{ readonly node: number; readonly of: GraphMeta | null }>({ node: -1, of: null });
  useEffect(() => view.on("hover", (node) => hover({ node, of: meta })), [view, meta]);
  return hovered.of === meta ? hovered.node : -1;
}

export const HoverCard = memo(function HoverCard(props: HoverCardProps): ReactElement | null {
  const { previews, meta, view } = props;
  const node = useHovered(view, meta);
  const read = (): ReturnType<typeof previews.shown> => previews.shown("hover");
  const shown = useSyncExternalStore(previews.subscribe, read, read);
  if (meta === null || node < 0 || node >= meta.nodeCount) return null;
  const { x, y } = view.position(node);
  const camera = view.camera();
  const style = { left: x * camera.scale + camera.x + OFFSET, top: y * camera.scale + camera.y + OFFSET };
  return (
    <div className="gs-panel gs-card" aria-hidden="true" style={style}>
      <PreviewBody preview={previewOrOwn(meta, node, shown.id === meta.ids[node] ? shown.preview : null)} />
    </div>
  );
});
