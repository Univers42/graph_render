import { memo, useState, type ReactElement } from "react";

import type { StudioAction, StudioParam } from "../actions/context.ts";
import type { ArgValue, Args } from "../actions/registry.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { ControlFor } from "./controls/choose.tsx";
import { valuesOf } from "./draft.ts";

/** The name an ingest document is known by; a file that was picked brings its own. */
const NAME = "name";

export interface ActionFormProps {
  readonly studio: Studio;
  readonly action: StudioAction;
  readonly state: StudioState;
  /** What the form draws out of the state, as one string; see `draftedOf`. */
  readonly drawn: string;
}

interface ParamFieldProps {
  readonly state: StudioState;
  readonly spec: StudioParam;
  readonly named: boolean;
  readonly value: ArgValue;
  readonly disabled: boolean;
  readonly onDraft: (patch: Args) => void;
  readonly onCommit: (patch: Args) => void;
}

function ParamField(props: ParamFieldProps): ReactElement {
  const { state, spec, named, value, disabled, onDraft, onCommit } = props;
  return (
    <ControlFor
      spec={spec}
      value={value}
      choices={spec.choices?.(state) ?? []}
      disabled={disabled}
      named={named}
      onDraft={onDraft}
      onCommit={onCommit}
    />
  );
}

export function ActionFormBody(props: ActionFormProps): ReactElement {
  const { studio, action, state } = props;
  // The dock remounts this on the values' signature, so a change made from the console
  // shows here: the draft is a copy of what the state said when the form was made.
  const [draft, setDraft] = useState<Args>(() => valuesOf(action, state));
  const reason = action.available?.(state) ?? null;
  const named = action.params.some((spec) => spec.name === NAME);
  const onDraft = (patch: Args): void => setDraft((current) => ({ ...current, ...patch }));
  const onCommit = (patch: Args): void => {
    setDraft((current) => ({ ...current, ...patch }));
    void studio.dispatch(action.id, { ...draft, ...patch });
  };
  const body = action.params.length === 0
    ? <button type="button" className="gs-btn" disabled={reason !== null} onClick={() => onCommit({})}>{action.title}</button>
    : action.params.map((spec) => (
      <ParamField
        key={spec.name}
        state={state}
        spec={spec}
        named={named}
        value={draft[spec.name] ?? ""}
        disabled={reason !== null}
        onDraft={onDraft}
        onCommit={onCommit}
      />
    ));
  return (
    <div className="gs-action">
      {action.params.length > 0 && action.title !== action.section && (
        <div className="gs-action-title">{action.title}</div>
      )}
      <div className="gs-form">{body}</div>
      {reason !== null && <p className="gs-reason">{reason}</p>}
    </div>
  );
}

/**
 * WHY the form is compared on what it draws and not on the state it is handed: the dock
 * re-renders on every store change, and every form in it is a dozen elements — so a change
 * to the log, the selection or the search box would redraw about two hundred of them for
 * nothing. `drawn` is that comparison, already made by the dock for the remount key.
 */
export const ActionForm = memo(ActionFormBody, (before, after) => (
  before.studio === after.studio
  && before.action === after.action
  && before.drawn === after.drawn
));
