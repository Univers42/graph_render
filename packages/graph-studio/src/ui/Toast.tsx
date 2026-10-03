/** Above the graph: what the motor is doing, or what it refused and what to do about it. */
import { memo, type ReactElement } from "react";

import type { ShownError } from "../state/errors.ts";
import type { Running } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { Failure } from "./Failure.tsx";

export interface ToastProps {
  readonly studio: Studio;
  /** What is running now, and what refused: the two fields this panel draws. */
  readonly busy: readonly Running[];
  readonly error: ShownError | null;
}

/** Memoised: the toast is empty for most of a studio's life, and says so without re-rendering. */
export const Toast = memo(function Toast(props: ToastProps): ReactElement | null {
  const { studio, busy, error } = props;
  const running = busy[busy.length - 1] ?? null;
  if (running === null && error === null) return null;
  return (
    <div className="gs-panel gs-toast">
      {running !== null && (
        <div className="gs-busy">
          <span className="gs-busy-name">{running.command}</span>
          <button
            type="button"
            className="gs-btn"
            aria-label="Stop what the motor is doing"
            onClick={() => void studio.dispatch("view.cancel")}
          >
            Stop
          </button>
        </div>
      )}
      {error !== null && <Failure error={error} announced onDismiss={() => studio.dismiss()} />}
    </div>
  );
});