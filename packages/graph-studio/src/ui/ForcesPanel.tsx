/** The Forces section: four live sliders, Reset and Animate, and one line saying why they are off. */
import { useState, type ReactElement } from "react";

import type { StudioAction } from "../actions/context.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { sig3 } from "./names.ts";

/** WHY aria-disabled and not disabled: a disabled control leaves the Tab order, and the reason is only found by reaching it. */
const REASON_ID = "gs-forces-reason";

export interface ForcesPanelProps {
  readonly studio: Studio;
  readonly state: StudioState;
}

interface SliderProps {
  readonly studio: Studio;
  readonly action: StudioAction;
  readonly state: StudioState;
  readonly disabled: boolean;
}

/** Applies on every change, not on release: a force slider that waits for the pointer is not live. */
function Slider(props: SliderProps): ReactElement | null {
  const { studio, action, state, disabled } = props;
  const spec = action.params[0];
  const [draft, setDraft] = useState<number | null>(null);
  if (spec === undefined) return null;
  const shown = draft ?? Number(spec.value(state));
  const change = (next: number): void => {
    if (disabled) return;
    setDraft(next);
    void studio.dispatch(action.id, { [spec.name]: next });
  };
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <span className="gs-row">
        <input
          className="gs-range"
          type="range"
          min={spec.min}
          max={spec.max}
          step={spec.step}
          value={String(shown)}
          aria-disabled={disabled}
          aria-describedby={REASON_ID}
          onChange={(event) => change(Number(event.target.value))}
        />
        <span className="gs-value">{sig3(shown)}</span>
      </span>
    </label>
  );
}

interface ButtonsProps {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly animate: StudioAction | undefined;
  readonly reset: StudioAction | undefined;
  readonly disabled: boolean;
}

function Buttons(props: ButtonsProps): ReactElement {
  const { studio, state, animate, reset, disabled } = props;
  return (
    <div className="gs-row">
      {animate !== undefined && (
        <label className="gs-field">
          <span className="gs-field-label">{animate.title}</span>
          <input
            className="gs-check"
            type="checkbox"
            role="switch"
            aria-disabled={disabled}
            aria-describedby={REASON_ID}
            defaultChecked={animate.params[0]?.value(state) === true}
            onChange={(event) => {
              if (!disabled) void studio.dispatch(animate.id, { on: event.target.checked });
            }}
          />
        </label>
      )}
      {reset !== undefined && (
        <button
          type="button"
          className="gs-btn"
          aria-disabled={disabled}
          aria-describedby={REASON_ID}
          onClick={() => {
            if (!disabled) void studio.dispatch(reset.id, {});
          }}
        >
          {reset.title}
        </button>
      )}
    </div>
  );
}

export function ForcesPanel(props: ForcesPanelProps): ReactElement {
  const { studio, state } = props;
  const actions = studio.registry.actions.filter((action) => action.section === "Forces");
  const reason = actions.map((action) => action.available?.(state) ?? null).find((why) => why !== null) ?? null;
  const disabled = reason !== null;
  const knobs = actions.filter((action) => action.params[0]?.control === "slider");
  const reset = actions.find((action) => action.id === "forces.reset");
  const animate = actions.find((action) => action.id === "forces.animate");
  return (
    <div className="gs-actions gs-forces">
      {knobs.map((action) => <Slider key={action.id} studio={studio} action={action} state={state} disabled={disabled} />)}
      <Buttons studio={studio} animate={animate} reset={reset} state={state} disabled={disabled} />
      {reason !== null && <p className="gs-reason" id={REASON_ID} role="status">{reason}</p>}
    </div>
  );
}
