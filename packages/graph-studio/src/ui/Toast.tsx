/** Above the graph: what the motor is doing, or what it refused and what to do about it. */
import type { ReactElement } from "react";

import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { Failure } from "./Failure.tsx";

export interface ToastProps {
  readonly studio: Studio;
  readonly state: StudioState;
}

export function Toast(props: ToastProps): ReactElement | null {
  const { studio, state } = props;
  const running = state.busy[state.busy.length - 1] ?? null;
  if (running === null && state.error === null) return null;
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
      {state.error !== null && <Failure error={state.error} announced onDismiss={() => studio.dismiss()} />}
    </div>
  );
}
