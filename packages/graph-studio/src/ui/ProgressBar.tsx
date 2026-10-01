/**
 * The progress bar at the top of the canvas: one element, one width, no state of its own.
 *
 * It subscribes to the bar's store and not to the studio's, so a settle at sixty frames a
 * second repaints this strip and nothing else. The element stays in the document and its
 * width is what changes: an element that mounts and unmounts per frame is a layout storm,
 * and the gate reads a width, not an existence.
 */
import { useSyncExternalStore, type CSSProperties, type ReactElement } from "react";

import type { Bar } from "./progress.ts";

export interface ProgressBarProps {
  readonly bar: () => Bar;
  readonly onBar: (handler: (bar: Bar) => void) => () => void;
}

const BAR_STYLE: CSSProperties = { width: "100%", height: "100%" };

export function ProgressBar(props: ProgressBarProps): ReactElement {
  const { bar, onBar } = props;
  const shown = useSyncExternalStore(onBar, bar, bar);
  const fraction = shown.fraction;
  return (
    <div className="gs-progress" role="progressbar" aria-label="Progress" aria-hidden={!shown.visible} hidden={!shown.visible}>
      <span
        className={fraction === null ? "gs-progress-fill gs-progress-sweep" : "gs-progress-fill"}
        style={fraction === null ? BAR_STYLE : { ...BAR_STYLE, width: `${Math.round(fraction * 100)}%` }}
      />
      <span className="gs-progress-label">{shown.label}</span>
    </div>
  );
}