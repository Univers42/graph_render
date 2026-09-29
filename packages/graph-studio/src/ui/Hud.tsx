/**
 * The HUD: one line under the graph. The numbers the view measures are written straight
 * into one element by the frame handler, at most four times a second, so that a graph that
 * is being panned does not re-render the chrome on every frame it draws.
 */
import { useEffect, useRef, useState, type ReactElement, type RefObject } from "react";

import type { View, ViewStats } from "../../../graph-render/src/view.ts";
import type { StudioState } from "../state/model.ts";
import { digest8, frameLine, ms } from "./names.ts";
import { due } from "./throttle.ts";

const EVERY = 250;
const LOOK_AGAIN = 500;

export interface HudProps {
  readonly state: StudioState;
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

export function Hud(props: HudProps): ReactElement {
  const { state, view } = props;
  const { first, line } = useFrameLine(view);
  const { run } = state;
  return (
    <div className="gs-panel gs-hud">
      <span className="gs-hud-frame" ref={line}>{first}</span>
      {run !== null && <span className="gs-muted">{` · layout ${ms(run.layoutMs)} · ${digest8(run.digest)}`}</span>}
    </div>
  );
}
