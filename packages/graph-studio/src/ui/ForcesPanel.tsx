/** The Forces section: nine live sliders, Spread and Compact, Animate, Pause, Resume and Reset, and one line saying why they are off. */
import { memo, useEffect, useState, useSyncExternalStore, type ReactElement } from "react";

import type { StudioAction } from "../actions/context.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { type BarStore, frameScheduler, throttledBar } from "./frameThrottle.ts";
import { sig3 } from "./names.ts";
import type { Bar } from "./progress.ts";

/** WHY aria-disabled and not disabled: a disabled control leaves the Tab order, and the reason is only found by reaching it. */
const REASON_ID = "gs-forces-reason";

export const PANEL_ID = "gs-dock-forces";

export interface ForcesPanelProps {
  readonly studio: Studio;
  readonly state: StudioState;
  /** The live loop's store, read by the one line below and by nothing else here. */
  readonly bar: BarStore;
}

/** What one line of the bar says: a settle has a fraction, a batch has only a count. */
function line(bar: Bar): string {
  const fraction = bar.fraction;
  return fraction === null ? bar.label : `${bar.label} ${Math.round(fraction * 100)}%`;
}

/**
 * The one line that reads the live loop's store, and the only thing here that redraws on it.
 *
 * WHY a version and not the bar itself: the redraw is a redraw of the store as it is when
 * React gets round to it, so a frame that lands between the publish and the draw is the one
 * on screen. Holding the published value would show the frame behind it instead.
 *
 * WHY a throttled subscription and not `useSyncExternalStore`: a settle publishes about once
 * every 16 ms, and that hook redraws per publish, which is how a panel of sliders and buttons
 * came to be redrawn sixty times a second to keep one number current.
 */
function BarLine(props: { readonly store: BarStore }): ReactElement | null {
  const { store } = props;
  const [, redraw] = useState(0);
  useEffect(() => throttledBar(store, frameScheduler(), () => redraw((n) => n + 1)), [store]);
  const shown = store.bar();
  // WHY nothing at all when the bar is hidden: the reason below already says the loop cannot
  // run, and an empty padded strip under the sliders reads as a control that failed.
  if (!shown.visible) return null;
  return (
    <div className="gs-busy">
      <span className="gs-busy-name">{line(shown)}</span>
    </div>
  );
}

interface SliderProps {
  readonly studio: Studio;
  readonly action: StudioAction;
  readonly state: StudioState;
  readonly disabled: boolean;
}

/**
 * Applies on every change, not on release: a force slider that waits for the pointer is not live.
 *
 * WHY the draft is dropped when the knob moves on its own: a preset or Reset sets the knob
 * the user last dragged, and a draft held from that drag would keep the thumb where it was.
 */
function Slider(props: SliderProps): ReactElement | null {
  const { studio, action, state, disabled } = props;
  const spec = action.params[0];
  const value = spec === undefined ? Number.NaN : Number(spec.value(state));
  const [draft, setDraft] = useState<number | null>(null);
  const [seen, setSeen] = useState(value);
  if (!Object.is(value, seen)) {
    setSeen(value);
    setDraft(null);
  }
  if (spec === undefined) return null;
  const shown = draft ?? value;
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
    <>
      <div className="gs-row">
        <Button studio={studio} action={at("forces.spread")} disabled={disabled} />
        <Button studio={studio} action={at("forces.compact")} disabled={disabled} />
      </div>
      <div className="gs-row">
        <Button studio={studio} action={at("forces.animate")} disabled={disabled} />
        <Button studio={studio} action={at("forces.pause")} disabled={disabled} />
        <Button studio={studio} action={at("forces.resume")} disabled={disabled} />
        <Button studio={studio} action={at("forces.reset")} disabled={disabled} />
      </div>
    </>
  );
}

/**
 * Everything the panel draws that the bridge owns, as one string: the reason, and the value
 * of every parameter. `available()` reads the last state the worker reported and a knob's
 * `value` reads the knobs it echoed, so both arrive on the bar's store and nowhere else.
 *
 * WHY a string and not an object: `useSyncExternalStore` compares the snapshot with `Object.is`,
 * so a rebuilt object would redraw the section on every frame, which is what this whole
 * change exists to stop. A frame changes the fraction and nothing below, and the string with
 * it.
 */
export function drawnOf(actions: readonly StudioAction[], state: StudioState): string {
  const reason = actions.map((action) => action.available?.(state) ?? null).find((why) => why !== null) ?? null;
  const values = actions.flatMap((action) => action.params.map((param) => String(param.value(state))));
  return `${reason ?? ""}|${values.join(",")}`;
}

export function ForcesPanelBody(props: ForcesPanelProps): ReactElement {
  const { studio, state, bar } = props;
  const actions = studio.registry.actions.filter((action) => action.section === "Forces");
  // WHY the panel subscribes at all: the worker answers on this store and on no other, so
  // without it the reason line keeps saying "not asked yet" until something else redraws.
  useSyncExternalStore(bar.onBar, () => drawnOf(actions, state), () => drawnOf(actions, state));
  const reason = actions.map((action) => action.available?.(state) ?? null).find((why) => why !== null) ?? null;
  const disabled = reason !== null;
  const knobs = actions.filter((action) => action.params[0]?.control === "slider");
  return (
    <div className="gs-actions gs-forces" id="gs-forces">
      {knobs.map((action) => <Slider key={action.id} studio={studio} action={action} state={state} disabled={disabled} />)}
      <Buttons studio={studio} actions={actions} disabled={disabled} />
      <BarLine store={bar} />
      {reason !== null && <p className="gs-reason" id={REASON_ID} role="status">{reason}</p>}
    </div>
  );
}

function forcesOf(studio: Studio): readonly StudioAction[] {
  return studio.registry.actions.filter((action) => action.section === "Forces");
}

/**
 * WHY the panel is compared on what it draws: the dock re-renders on every store change and
 * the panel is sliders, buttons and a reason — none of which read the log, the
 * selection or the search box. The bar is the other half: it changes about sixty times a
 * second during a settle, and the snapshot above is what tells the two apart.
 */
export const ForcesPanel = memo(ForcesPanelBody, (before, after) => (
  before.studio === after.studio
  && before.bar === after.bar
  && drawnOf(forcesOf(before.studio), before.state) === drawnOf(forcesOf(after.studio), after.state)
));