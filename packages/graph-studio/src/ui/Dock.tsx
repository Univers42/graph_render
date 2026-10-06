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

/** One id per section: lowercased, and with the gaps a two-word name has closed up, because
 *  an `id` is one token and a space would make it two. */
function sectionId(name: string): string {
  return `gs-dock-${name.toLowerCase().replaceAll(" ", "-")}`;
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
      {/* WHY a shut section draws nothing: the dock redraws on every store change, and every
          action's value and reason in every section was worked out for a body nobody saw. */}
      <div className="gs-section-body" id={id} aria-labelledby={`${id}-head`} hidden={!open}>{open && children}</div>
    </>
  );
}

interface PanelProps {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly name: string;
  readonly bar: DockProps["bar"];
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
 * What a section shows of its own, and whether its actions are listed under it. The forces
 * panel brings its own controls for the force actions, so it is the whole of that section;
 * the layout's panel is a set of controls beside the one action that puts the defaults back.
 */
const PANELS: Readonly<Record<string, "alone" | "beside">> = {
  Forces: "alone",
  [PARAMS_SECTION]: "beside",
};

function panelOf(props: PanelProps): ReactNode {
  const { studio, state, name, bar } = props;
  if (name === "Forces") return <ForcesPanel studio={studio} state={state} bar={bar} />;
  if (name === PARAMS_SECTION) {
    // WHY the key is the layout and its values: the panel's draft is what the controls show,
    // so a run that changed a value has to rebuild it from the state that landed.
    return <LayoutParamsPanel key={paramsKey(state, specsOf(state))} studio={studio} state={state} />;
  }
  return null;
}

function Body(props: PanelProps): ReactElement {
  const { studio, state, name } = props;
  return (
    <>
      {panelOf(props)}
      {PANELS[name] !== "alone" && <Actions studio={studio} state={state} name={name} />}
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
