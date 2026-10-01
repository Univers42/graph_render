/** The Forces section: four live sliders, Animate, Reset, Pause and Resume, and one line saying why they are off. */
import { useState, useSyncExternalStore, type ReactElement } from "react";

import type { StudioAction } from "../actions/context.ts";
import type { LiveBridge } from "../motor/bridge.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { sig3 } from "./names.ts";

/** WHY aria-disabled and not disabled: a disabled control leaves the Tab order, and the reason is only found by reaching it. */
const REASON_ID = "gs-forces-reason";

export const PANEL_ID = "gs-dock-forces";

export interface ForcesPanelProps {
  readonly studio: Studio;
  readonly state: StudioState;
  /** The live loop's store: read only for the redraw, so the reason is never stale. */
  readonly bar: Pick<LiveBridge, "bar" | "onBar">;
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
  readonly actions: readonly StudioAction[];
  readonly disabled: boolean;
}

function run(studio: Studio, disabled: boolean, action: StudioAction | undefined): void {
  if (disabled || action === undefined) return;
  void studio.dispatch(action.id, {});
}

interface ButtonProps {
  readonly studio: Studio;
  readonly action: StudioAction | undefined;
  readonly disabled: boolean;
}

/** Animate is a button, not a switch: pressing it restarts the settle from random positions. */
function Button(props: ButtonProps): ReactElement | null {
  const { studio, action, disabled } = props;
  if (action === undefined) return null;
  return (
    <button
      type="button"
      className="gs-btn"
      aria-disabled={disabled}
      aria-describedby={REASON_ID}
      onClick={() => run(studio, disabled, action)}
    >
      {action.title}
    </button>
  );
}

function Buttons(props: ButtonsProps): ReactElement {
  const { studio, actions, disabled } = props;
  const at = (id: string): StudioAction | undefined => actions.find((action) => action.id === id);
  return (
    <div className="gs-row">
      <Button studio={studio} action={at("forces.animate")} disabled={disabled} />
      <Button studio={studio} action={at("forces.pause")} disabled={disabled} />
      <Button studio={studio} action={at("forces.resume")} disabled={disabled} />
      <Button studio={studio} action={at("forces.reset")} disabled={disabled} />
    </div>
  );
}

export function ForcesPanel(props: ForcesPanelProps): ReactElement {
  const { studio, state, bar } = props;
  // WHY the subscription is here and its value unused: whether the loop can run is the
  // bridge's own state, not the studio's, so without this the panel renders once with
  // "not asked yet" and never redraws when the worker's answer arrives.
  useSyncExternalStore(bar.onBar, bar.bar, bar.bar);
  const actions = studio.registry.actions.filter((action) => action.section === "Forces");
  const reason = actions.map((action) => action.available?.(state) ?? null).find((why) => why !== null) ?? null;
  const disabled = reason !== null;
  const knobs = actions.filter((action) => action.params[0]?.control === "slider");
  return (
    <div className="gs-actions gs-forces" id="gs-forces">
      {knobs.map((action) => <Slider key={action.id} studio={studio} action={action} state={state} disabled={disabled} />)}
      <Buttons studio={studio} actions={actions} disabled={disabled} />
      {reason !== null && <p className="gs-reason" id={REASON_ID} role="status">{reason}</p>}
    </div>
  );
}