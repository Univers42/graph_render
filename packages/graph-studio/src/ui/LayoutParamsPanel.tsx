/**
 * The Layout settings panel: one control per parameter the motor publishes for the layout on
 * screen. The way back to the defaults it published is the section's own action, beside these.
 *
 * The panel holds no schema of its own. Every name, bound and step is what the motor sent
 * (`state.schemas`), so the studio's source never names a parameter and cannot fall out of step
 * with the registry. The controls are the dock's own (`controlOf`, `ControlFor`), so a float is
 * a slider, an int a number field and a bool a switch without any of this being said here.
 *
 * A change re-runs the layout: the values pile up and one run happens on the next frame, so a
 * slider dragged across the screen costs one run and not one per pixel. That run cancels
 * whatever is in flight on the way (`pipeline.apply`), which is the same path `view.cancel`
 * uses. Putting the values back is the section's own action, next to these controls.
 */
import { useRef, useState, type ReactElement } from "react";

import { SET_MANY_ID, specsOf } from "../actions/params.ts";
import type { Args } from "../actions/registry.ts";
import type { StudioState } from "../state/model.ts";
import type { ParamValues } from "../state/settings.ts";
import type { Studio } from "../studio/studio.ts";
import { ControlFor } from "./controls/choose.tsx";
import { frameScheduler } from "./frameThrottle.ts";
import { heldValue, rowsOf, valuesOf, valuesPatch } from "./paramSpecs.ts";
import { type OneRun, oneRunPerFrame } from "./oneRun.ts";

export interface LayoutParamsPanelProps {
  readonly studio: Studio;
  readonly state: StudioState;
}

/** Why the panel cannot set a value now, or `null`; the action's own rule, read from the registry. */
function reasonOf(studio: Studio, id: string, state: StudioState): string | null {
  return studio.registry.find(id)?.available?.(state) ?? null;
}

export function LayoutParamsPanel(props: LayoutParamsPanelProps): ReactElement {
  const { studio, state } = props;
  const specs = specsOf(state);
  const rows = rowsOf(specs);
  const reason = reasonOf(studio, SET_MANY_ID, state);
  // The draft is what the controls show while a value is being moved; the store only catches up
  // when the run lands, and the dock remounts this panel on that change (`paramsKey`).
  const [draft, setDraft] = useState<ParamValues>(() => valuesOf(state, specs));
  const pending = useRef<ParamValues>({});
  const once = useRef<OneRun | null>(null);
  const commit = (patch: Args): void => {
    const moved = valuesPatch(patch);
    setDraft((current) => ({ ...current, ...moved }));
    pending.current = { ...pending.current, ...moved };
    once.current ??= oneRunPerFrame(frameScheduler(), () => {
      const values = pending.current;
      pending.current = {};
      void studio.dispatch(SET_MANY_ID, { values: JSON.stringify(values) });
    });
    once.current.ask();
  };
  return (
    <div className="gs-action">
      <div className="gs-action-title">{state.settings.layout}</div>
      <div className="gs-form">
        {rows.map((row) => (
          <ControlFor
            key={row.spec.name}
            spec={row.knob}
            value={heldValue(draft, row.spec)}
            choices={[]}
            disabled={reason !== null}
            named={false}
            onDraft={(patch) => setDraft((current) => ({ ...current, ...valuesPatch(patch) }))}
            onCommit={commit}
          />
        ))}
      </div>
      {reason !== null && <p className="gs-reason">{reason}</p>}
    </div>
  );
}
