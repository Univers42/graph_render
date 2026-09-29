/**
 * The HUD: one line under the graph. The numbers the view measures are written straight
 * into one element by the frame handler, at most four times a second, so that a graph that
 * is being panned does not re-render the chrome on every frame it draws.
 */
import { useEffect, useRef, useState, type ReactElement } from "react";

import type { View, ViewStats } from "../../../graph-render/src/view.ts";
import type { StudioState } from "../state/model.ts";
import { digest8, ms } from "./names.ts";
import { due } from "./throttle.ts";

const EVERY = 250;

export interface HudProps {
  readonly state: StudioState;
  readonly view: Pick<View, "stats" | "on">;
}

/** The view reports 0 while nothing moves: it paints on demand, and a parked graph has no rate. */
function rateOf(fps: number): string {
  return fps > 0 ? `${Math.round(fps)} fps` : "idle";
}

function frameLine(stats: ViewStats): string {
  return `${stats.nodes} n · ${stats.edges} e · ${rateOf(stats.fps)} · ${ms(stats.frameMs)} · ${stats.backend}`;
}

export function Hud(props: HudProps): ReactElement {
  const { state, view } = props;
  // What the first paint shows. After that the frame handler owns this element's text.
  const [first] = useState(() => frameLine(view.stats()));
  const line = useRef<HTMLSpanElement | null>(null);
  const last = useRef(-Infinity);
  useEffect(() => view.on("frame", (stats) => {
    const now = performance.now();
    if (!due(last.current, now, EVERY)) return;
    last.current = now;
    const element = line.current;
    if (element !== null) element.textContent = frameLine(stats);
  }), [view]);
  const { run } = state;
  return (
    <div className="gs-panel gs-hud">
      <span className="gs-hud-frame" ref={line}>{first}</span>
      {run !== null && <span className="gs-muted">{` · layout ${ms(run.layoutMs)} · ${digest8(run.digest)}`}</span>}
    </div>
  );
}
