/**
 * The force knobs kept in the settings, so they are saved and recalled with everything else,
 * and the motor kept in step with them.
 *
 * The motor makes a new force session at its own defaults for every graph it loads, so the
 * knobs the settings hold are pushed again once the new graph has a force run to settle. A
 * set from the panel or the console goes to the motor at once, as before, and into the store.
 */
import { MIN_SCREEN_RADIUS } from "../../../graph-render/src/canvas2d/nodes.ts";
import type { Frame } from "../../../graph-render/src/frame.ts";
import { styleFrom } from "../../../graph-render/src/style.ts";
import type { ForceLink } from "../actions/forces.ts";
import { styleInputOf } from "../look/styleOf.ts";
import { DEFAULT_KNOBS, type ForceKnobs, settlesLive } from "../motor/live.ts";
import type { GraphSummary } from "../motor/protocol.ts";
import type { ViewFace } from "../studio/pipeline.ts";
import { sameKnobs } from "./forces.ts";
import type { RunSummary, StudioState } from "./model.ts";
import { withSettings } from "./settings.ts";
import type { Store } from "./store.ts";

export interface KeptForces {
  readonly link: ForceLink;
  readonly stop: () => void;
}

interface Held {
  graph: GraphSummary | null;
  /** The run on screen when the graph changed: it belongs to the graph before. */
  before: RunSummary | null;
  /** What the motor's session for this graph was last given. */
  knobs: ForceKnobs;
  scale: number;
}

function largest(column: Float32Array): number {
  let top = 0;
  for (let i = 0; i < column.length; i += 1) top = Math.max(top, column[i] ?? 0);
  return top;
}

/** The largest node extent in world units, read the way the painter reads it (`scene.ts`). */
function largestExtent(state: StudioState, frame: Frame): number | null {
  if (frame.r !== null) return largest(frame.r);
  if (frame.w !== null && frame.h !== null) return Math.max(largest(frame.w), largest(frame.h)) / 2;
  const { meta, analysis, reveal } = state;
  if (meta === null) return null;
  const { appearance, filter, groups } = state.settings;
  return styleFrom(styleInputOf({ meta, appearance, filter, groups, analysis, reveal })).maxRadius;
}

/**
 * The largest disc on screen as a radius in layout units, or `null` when nothing is drawn.
 * A disc is never drawn under MIN_SCREEN_RADIUS pixels, which is `MIN_SCREEN_RADIUS / scale`
 * in the world. World units are layout units once a live settle has painted: its frames go
 * to the view as the motor's own coordinates (`docs/measurements/ux-forces-full.md`).
 *
 * Ponytail: on a frozen layout that was never settled live, the world is the motor's units
 * times the frame's `factor`, so the radius is off by that factor until a settle repaints.
 * A 3D frame is sized by the 2D camera's scale, which ignores depth.
 */
export function drawnRadius(state: StudioState, frame: Frame, scale: number): number | null {
  if (state.meta === null || frame.nodeCount === 0 || !(scale > 0)) return null;
  const extent = largestExtent(state, frame);
  return extent === null ? null : Math.max(MIN_SCREEN_RADIUS / scale, extent);
}

/**
 * Pushes the settings' knobs to a session that has not had them: a new graph's session, once
 * a force run on it exists, and any change that came from outside the link (an import, a
 * recalled source) while a force run is on screen.
 */
function follow(at: StudioState, held: Held, push: (knobs: ForceKnobs) => void): void {
  if (at.graph !== held.graph) {
    held.graph = at.graph;
    held.before = at.run;
    held.knobs = DEFAULT_KNOBS;
  }
  if (at.run === null || at.run === held.before || !settlesLive(at.run.layoutId)) return;
  if (!sameKnobs(at.settings.forces, held.knobs)) push(at.settings.forces);
}

export function keepForces(store: Store<StudioState>, inner: ForceLink, view: Pick<ViewFace, "on" | "frame">): KeptForces {
  const held: Held = { graph: null, before: null, knobs: DEFAULT_KNOBS, scale: 1 };
  const push = (knobs: ForceKnobs): void => {
    held.knobs = knobs;
    inner.set(knobs);
  };
  const unstore = store.subscribe(() => follow(store.get(), held, push));
  const uncamera = view.on("camera", (camera) => { held.scale = camera.scale; });
  const link: ForceLink = {
    ...inner,
    knobs: (state) => (state ?? store.get()).settings.forces,
    set: (knobs) => {
      push(knobs);
      store.update((state) => ({ ...state, settings: withSettings(state.settings, { forces: knobs }) }));
    },
    drawn: () => drawnRadius(store.get(), view.frame(), held.scale),
  };
  return { link, stop: () => { unstore(); uncamera(); } };
}
