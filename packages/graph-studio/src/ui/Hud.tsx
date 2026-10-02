/**
 * The HUD: one line under the graph. The numbers the view measures are written straight
 * into one element by the frame handler, at most four times a second, so that a graph that
 * is being panned does not re-render the chrome on every frame it draws.
 */
import { memo, useEffect, useRef, useState, type ReactElement, type RefObject } from "react";

import type { View, ViewStats } from "../../../graph-render/src/view.ts";
import type { RunSummary } from "../state/model.ts";
import { digest8, frameLine, ms } from "./names.ts";
import { due } from "./throttle.ts";

const EVERY = 250;
const LOOK_AGAIN = 500;

export interface HudProps {
  /** The one slice the HUD draws; the rest of the state is not its news. */
  readonly run: RunSummary | null;
  readonly view: Pick<View, "stats" | "on">;
}

interface FrameLine {
  /** What the first paint shows. After that the frame handler owns the element's text. */
  readonly first: string;
  readonly line: RefObject<HTMLSpanElement | null>;
}

/**
 * Ponytail: the view says nothing when it parks, so the line looks again half a second
 * after a frame, and keeps looking while the view moves. It still reads a rate for up to
 * 900 ms after the last frame of a move (the 400 ms the view waits, then these 500).
 * Nothing runs once it has read `idle`.
 */
function useFrameLine(view: HudProps["view"]): FrameLine {
  const [first] = useState(() => frameLine(view.stats(), 0));
  const line = useRef<HTMLSpanElement | null>(null);
  useEffect(() => {
    let written = -Infinity;
    let moved = 0;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const write = (stats: ViewStats): void => {
      if (stats.fps > 0) moved = stats.fps;
      if (line.current !== null) line.current.textContent = frameLine(stats, moved);
    };
    const look = (): void => {
      const stats = view.stats();
      write(stats);
      timer = stats.fps > 0 ? setTimeout(look, LOOK_AGAIN) : null;
    };
    const off = view.on("frame", (stats) => {
      timer ??= setTimeout(look, LOOK_AGAIN);
      const now = performance.now();
      if (!due(written, now, EVERY)) return;
      written = now;
      write(stats);
    });
    return () => {
      off();
      if (timer !== null) clearTimeout(timer);
    };
  }, [view]);
  return { first, line };
}

/**
 * WHY the badge reads the run's dim and not the layout id: `dim` comes off the decoded
 * snapshot, so it is the z column's own presence — the same thing the painter branched on.
 * A layout id could promise 3D and deliver a flat graph; this cannot.
 *
 * WHY the leading space is in the text and not only in the CSS: the frame line beside it is
 * written into its own element, so the badge's own margin is the only gap between two pieces
 * of text — and a margin is not a space to anything that reads the line, a screen reader
 * included. It reads "canvas2d 3D" here and "canvas2d3D" without it.
 */
function SpaceBadge({ dim }: { readonly dim: number }): ReactElement | null {
  if (dim !== 1) return null;
  return (
    <span className="gs-badge" title="This layout placed its nodes in 3D. Drag to turn it, wheel to zoom, right-drag to pan.">
      {" 3D"}
    </span>
  );
}

/** Memoised: the HUD draws the last run and what the view measured, and nothing else. */
export const Hud = memo(function Hud(props: HudProps): ReactElement {
  const { run, view } = props;
  const { first, line } = useFrameLine(view);
  return (
    <div className="gs-panel gs-hud">
      <span className="gs-hud-frame" ref={line}>{first}</span>
      {run !== null && <SpaceBadge dim={run.dim} />}
      {run !== null && <span className="gs-muted">{` · layout ${ms(run.layoutMs)} · ${digest8(run.digest)}`}</span>}
    </div>
  );
});
