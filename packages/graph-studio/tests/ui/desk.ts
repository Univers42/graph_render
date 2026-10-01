/** A studio to draw chrome over: a motor that refuses everything, and a state built by hand. */
import type { ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";

import type { ViewStats } from "../../../graph-render/src/view.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { LiveBridge } from "../../src/motor/bridge.ts";
import { HIDDEN } from "../../src/ui/progress.ts";
import { type StudioState, initialState } from "../../src/state/model.ts";
import { desk, refusingClient, type Desk } from "../desk.ts";
import { CATALOG, DIGEST, DRAWN, META } from "../drawn.ts";

export { CATALOG, DIGEST, DRAWN, META };

export { refusingClient };

export const IDLE: StudioState = initialState();

export const STATS: ViewStats = {
  backend: "canvas2d", nodes: 3, edges: 2, drawnNodes: 3, drawnEdges: 2, drawnLabels: 2, drawnArrows: 0, arrowSize: 0, curvedEdges: 0, strokeWidth: 0,
  draws: 12, strokeCalls: 1, edgeStyles: 1, mixedEdges: 0, gradientStrokes: 0, arrowFills: 0, glowFills: 0, spritesRasterised: 0, layoutRuns: 1, frameMs: 4.2, fps: 60, frames: 42,
};

export function studioWith(state: StudioState = DRAWN): Desk {
  const made = desk(refusingClient());
  made.studio.store.set(state);
  return made;
}

/** The parts of a view the chrome uses, recording nothing: effects do not run here. */
export function fakeView(pinned: readonly number[] = []): Pick<View, "stats" | "on" | "focus" | "select" | "camera" | "position" | "hide" | "togglePin" | "pinned"> {
  return {
    camera: () => ({ x: 0, y: 0, scale: 1 }),
    position: () => ({ x: 0, y: 0 }),
    hide: () => undefined,
    togglePin: () => undefined,
    pinned: () => pinned,
    stats: () => STATS,
    focus: () => undefined,
    select: () => undefined,
    on: () => () => undefined,
  };
}

export function markup(element: ReactElement): string {
  return renderToStaticMarkup(element);
}

/** The live loop's store, standing still: a hidden bar that nothing pushes to. */
export function fakeBar(): Pick<LiveBridge, "bar" | "onBar"> {
  return { bar: () => HIDDEN, onBar: () => () => undefined };
}
