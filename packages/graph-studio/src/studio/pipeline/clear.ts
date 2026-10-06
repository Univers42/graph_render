/**
 * What a fresh load that failed leaves behind: an empty drawing, and a store that names the graph
 * the drawing was made from. Both halves are one patch, so a reader of the store never sees the
 * graph of the failed run with the frame of the one before it.
 */
import type { Frame } from "../../../../graph-render/src/frame.ts";
import { EMPTY_FRAME } from "../../../../graph-render/src/scene.ts";
import type { AnalysisReport, GraphSummary } from "../../motor/protocol.ts";
import type { StudioState } from "../../state/model.ts";
import { type Source, withSettings } from "../../state/settings.ts";
import type { Store } from "../../state/store.ts";

/** The bytes of the last drawing, with the frame they decoded to; `null` when nothing is held. */
export interface Held {
  readonly bytes: Uint8Array;
  readonly ends: Frame;
}

/**
 * What the store carried before a fresh load began, read in one pass before the load is asked for.
 * A load that fails can have set the graph, the analysis and the source, and this is what they were
 * before that: nothing here is invented, it is the state the drawing on screen was made from.
 */
export interface Before {
  readonly graph: GraphSummary | null;
  readonly analysis: AnalysisReport | null;
  readonly source: Source;
}

/** The parts of the pipeline's rig that emptying the drawing needs; its own rig satisfies this. */
export interface Clearable {
  held: Held | null;
  readonly view: { setFrame(frame: Frame): void };
  readonly store: Store<StudioState>;
}

export function beforeOf(state: StudioState): Before {
  return { graph: state.graph, analysis: state.analysis, source: state.settings.source };
}

/**
 * WHY the frame goes and the graph comes back: a fresh load that never drew left the drawing on
 * screen exactly as it was, so the frame is emptied (it is of the graph the load was replacing) and
 * the store is put back to the graph that frame was made from. Left at the failed run's values it
 * would claim a graph nothing is drawn from, and the next apply would take the failed document for
 * the one already open and never load it again.
 */
export function clear(rig: Clearable, before: Before): void {
  rig.held = null;
  rig.view.setFrame(EMPTY_FRAME);
  rig.store.update((state) => ({
    ...state,
    meta: null, run: null, selected: -1, selection: [], reveal: null,
    graph: before.graph, analysis: before.analysis,
    settings: withSettings(state.settings, { source: before.source }),
  }));
}