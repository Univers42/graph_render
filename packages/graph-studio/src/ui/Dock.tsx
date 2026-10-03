/** The dock: what can be done to the graph, grouped, and why one group cannot be used now. */
import { useState, type ReactElement, type ReactNode } from "react";

import { DOCK_SECTIONS } from "../actions/all.ts";
import { PARAMS_SECTION, specsOf } from "../actions/params.ts";
import type { LiveBridge } from "../motor/bridge.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { ForcesPanel } from "./ForcesPanel.tsx";
import { ActionForm } from "./ActionForm.tsx";
import { LayoutParamsPanel } from "./LayoutParamsPanel.tsx";
import { drawnOf } from "./draft.ts";
import { paramsKey } from "./paramSpecs.ts";
import { useStudioState } from "./useStudio.ts";

/** One section open: with three, the dock was as tall as the page and covered the graph. */
const OPEN_AT_FIRST: readonly string[] = ["Layout"];
const BODY = "gs-dock-body";

export interface DockProps {
  readonly studio: Studio;
  readonly open: boolean;
  readonly onToggle: () => void;
  /** The live loop's store, so the Forces section redraws when the worker answers. */
  readonly bar: Pick<LiveBridge, "bar" | "onBar">;
}

interface SectionProps {
  readonly name: string;
  readonly open: boolean;
  readonly onToggle: () => void;
  readonly children: ReactNode;
}

function sectionId(name: string): string {
  return `gs-dock-${name.toLowerCase()}`;
}

function Section(props: SectionProps): ReactElement {
  const { name, open, onToggle, children } = props;
  const id = sectionId(name);
  return (
    <>
      <button
        type="button"
        className="gs-btn gs-section-head"
        id={`${id}-head`}
        aria-expanded={open}
        aria-controls={id}
        onClick={onToggle}
      >
        <span className="gs-head-name">{name}</span>
        <span aria-hidden="true">{open ? "▾" : "▸"}</span>
      </button>
      <div className="gs-section-body" id={id} aria-labelledby={`${id}-head`} hidden={!open}>{children}</div>
    </>
  );
}

function Actions(props: { readonly studio: Studio; readonly state: StudioState; readonly name: string }): ReactElement {
  const { studio, state, name } = props;
  return (
    <div className="gs-actions">
      {studio.registry.actions.filter((action) => action.section === name).map((action) => {
        const drawn = drawnOf(action, state);
        // WHY the id is in the key: three actions of one section take no parameters and so
        // share a signature, and React needs the key to be unique among the siblings.
        return <ActionForm key={`${action.id} ${drawn}`} studio={studio} action={action} state={state} drawn={drawn} />;
      })}
    </div>
  );
}

/**
 * The sections that draw something of their own before their actions: the live loop's bar and
 * the layout's own parameters. Everything else is the actions of the section and nothing else.
 */
function Body(props: {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly name: string;
  readonly bar: DockProps["bar"];
}): ReactElement {
  const { studio, state, name, bar } = props;
  const specs = specsOf(state);
  return (
    <>
      {name === "Forces" && <ForcesPanel studio={studio} state={state} bar={bar} />}
      {name === PARAMS_SECTION && (
        // WHY the key is the layout and its values: the panel's draft is what the controls
        // show, so a run that changed a value has to rebuild it from the state that landed.
        <LayoutParamsPanel key={paramsKey(state, specs)} studio={studio} state={state} />
      )}
      <Actions studio={studio} state={state} name={name} />
    </>
  );
}

export function Dock(props: DockProps): ReactElement {
  const { studio, open, onToggle, bar } = props;
  // WHY it subscribes itself: every action's value and every reason is read off the whole
  // state, so the dock is redrawn by any change to it — but nothing else in the chrome is.
  const state = useStudioState(studio);
  const [shown, setShown] = useState<readonly string[]>(OPEN_AT_FIRST);
  const flip = (name: string): void => {
    setShown((current) => current.includes(name) ? current.filter((other) => other !== name) : [...current, name]);
  };
  return (
    <div className="gs-panel gs-dock">
      <div className="gs-head">
        <button type="button" className="gs-btn gs-section-head" aria-expanded={open} aria-controls={BODY} onClick={onToggle}>
          <span className="gs-head-name">Controls</span>
          <span aria-hidden="true">{open ? "▾" : "▸"}</span>
        </button>
      </div>
      <div className="gs-dock-body" id={BODY} hidden={!open}>
        {DOCK_SECTIONS.map((name) => (
          <Section key={name} name={name} open={shown.includes(name)} onToggle={() => flip(name)}>
            <Body studio={studio} state={state} name={name} bar={bar} />
          </Section>
        ))}
      </div>
    </div>
  );
}
