/**
 * The two gestures that ask the host to open a node (verdict 11); the inspector's Open button is
 * the third. Each goes through the registry as `node.open`, so the host hears `node-open` from
 * one place whatever asked for it.
 */
import { localPoint } from "../../../graph-render/src/pointer.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Studio } from "../studio/studio.ts";
import type { OpenVia } from "./contract.ts";

export interface GestureDeps {
  readonly host: HTMLElement;
  readonly canvas: HTMLCanvasElement;
  readonly studio: Pick<Studio, "store" | "dispatch">;
  readonly view: Pick<View, "pick">;
}

function openNode(deps: GestureDeps, node: number, via: OpenVia): void {
  const id = deps.studio.store.get().meta?.ids[node];
  if (id !== undefined) void deps.studio.dispatch("node.open", { id, via });
}

/**
 * WHY the first node of the composed path and not `target`: an Enter typed into the console or
 * the search box, or pressed on a button, belongs to that control. Only a key the element itself
 * received, with nothing inside it focused, is the user asking about the selected node.
 */
function onEnter(deps: GestureDeps, event: KeyboardEvent): void {
  if (event.key !== "Enter" || event.repeat || event.composedPath()[0] !== deps.host) return;
  const { selection } = deps.studio.store.get();
  const only = selection.length === 1 ? selection[0] : undefined;
  if (only === undefined) return;
  event.preventDefault();
  openNode(deps, only, "enter");
}

/** Starts listening; returns what stops it. */
export function watchGestures(deps: GestureDeps): () => void {
  const onDouble = (event: MouseEvent): void => {
    const node = deps.view.pick(localPoint(deps.canvas, event));
    if (node >= 0) openNode(deps, node, "dblclick");
  };
  const onKey = (event: KeyboardEvent): void => onEnter(deps, event);
  deps.canvas.addEventListener("dblclick", onDouble);
  deps.host.addEventListener("keydown", onKey);
  return () => {
    deps.canvas.removeEventListener("dblclick", onDouble);
    deps.host.removeEventListener("keydown", onKey);
  };
}
