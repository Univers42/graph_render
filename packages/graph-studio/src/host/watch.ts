/**
 * The element's events, read off the studio's state and the view's hover: what changed, said
 * once, in ids (`docs/contract/host-api.md`, Events; verdict 10).
 *
 * Every event comes from a state change and not from the call that caused it, so a click, a box,
 * a typed `select` and a host's `selectNodes` all announce a selection the same way, and a call
 * that changes nothing announces nothing.
 */
import type { View } from "../../../graph-render/src/view.ts";
import type { ShownError } from "../state/errors.ts";
import type { GraphSummary } from "../motor/protocol.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { StudioState } from "../state/model.ts";
import type { Store } from "../state/store.ts";
import { firstOf } from "../studio/pipeline.ts";
import { type FrameScheduler, frameScheduler } from "../ui/frameThrottle.ts";
import { wireError } from "./contract.ts";
import { emit } from "./events.ts";
import type { Previews } from "./previews.ts";

export interface WatchDeps {
  readonly host: EventTarget;
  readonly store: Store<StudioState>;
  readonly view: Pick<View, "on">;
  readonly previews: Previews;
  readonly frames?: FrameScheduler;
}

interface Seen {
  graph: GraphSummary | null;
  /** The meta on screen when a new graph arrived: `graph-load` waits for a different one. */
  awaited: { readonly meta: GraphMeta | null } | null;
  announced: readonly string[];
  error: ShownError | null;
  hovered: string | null;
  node: number;
  cancelFrame: (() => void) | null;
}

function idsOf(state: StudioState): readonly string[] {
  const { meta } = state;
  return meta === null ? [] : state.selection.map((node) => meta.ids[node] ?? "");
}

function sameList(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((id, at) => id === b[at]);
}

function hoverTo(deps: WatchDeps, seen: Seen, id: string | null): void {
  if (seen.hovered === id) return;
  seen.hovered = id;
  emit(deps.host, "node-hover", { id });
  deps.previews.want("hover", id);
}

/**
 * A new graph: nothing asked about the last one is kept, and the pointer is over nothing yet.
 *
 * WHY the reset goes through the frame and not straight to `hoverTo`: a host that swaps the
 * graph and moves the pointer inside one frame would otherwise hear two `node-hover` (verdict
 * 10). The pointer is over nothing, so `seen.node` is -1 and a frame already waiting for the
 * hover announces the reset rather than the node the pointer left.
 */
function newGraph(deps: WatchDeps, seen: Seen, frames: FrameScheduler, state: StudioState): void {
  seen.graph = state.graph;
  seen.awaited = { meta: state.meta };
  deps.previews.clear();
  hovered(deps, seen, frames, -1);
}

function loaded(deps: WatchDeps, seen: Seen, state: StudioState): void {
  const { graph, meta } = state;
  if (seen.awaited === null || meta === null || meta === seen.awaited.meta || graph === null) return;
  seen.awaited = null;
  emit(deps.host, "graph-load", { nodes: graph.nodeCount, edges: graph.edgeCount, notes: [...firstOf(graph.notes)] });
}

function selected(deps: WatchDeps, seen: Seen, state: StudioState): void {
  const ids = idsOf(state);
  deps.previews.want("inspector", ids.length === 1 ? (ids[0] ?? null) : null);
  if (sameList(ids, seen.announced)) return;
  seen.announced = ids;
  emit(deps.host, "node-select", { ids: [...ids] });
}

function failed(deps: WatchDeps, seen: Seen, state: StudioState): void {
  if (state.error === seen.error) return;
  seen.error = state.error;
  if (state.error !== null) emit(deps.host, "graph-error", { error: wireError(state.error), message: state.error.detail });
}

function changed(deps: WatchDeps, seen: Seen, frames: FrameScheduler): void {
  const state = deps.store.get();
  if (state.graph !== seen.graph) newGraph(deps, seen, frames, state);
  loaded(deps, seen, state);
  selected(deps, seen, state);
  failed(deps, seen, state);
}

/**
 * At most one `node-hover` a frame (verdict 10): a pointer crossing ten nodes between two frames
 * is announced once, over the node it ended on.
 */
function hovered(deps: WatchDeps, seen: Seen, frames: FrameScheduler, node: number): void {
  seen.node = node;
  seen.cancelFrame ??= frames.next(() => {
    seen.cancelFrame = null;
    const meta = deps.store.get().meta;
    hoverTo(deps, seen, meta === null || seen.node < 0 ? null : (meta.ids[seen.node] ?? null));
  });
}

/** Starts announcing; returns what stops it. */
export function watchHost(deps: WatchDeps): () => void {
  const state = deps.store.get();
  const seen: Seen = {
    graph: state.graph, awaited: null, announced: idsOf(state), error: state.error, hovered: null, node: -1, cancelFrame: null,
  };
  const frames = deps.frames ?? frameScheduler();
  const unsubscribe = deps.store.subscribe(() => changed(deps, seen, frames));
  const unhover = deps.view.on("hover", (node) => hovered(deps, seen, frames, node));
  return () => {
    unsubscribe();
    unhover();
    seen.cancelFrame?.();
  };
}
