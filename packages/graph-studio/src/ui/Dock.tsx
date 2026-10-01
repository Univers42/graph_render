/** The dock: what can be done to the graph, grouped, and why one group cannot be used now. */
import { useState, type ReactElement, type ReactNode } from "react";

import { DOCK_SECTIONS } from "../actions/all.ts";
import type { LiveBridge } from "../motor/bridge.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { ForcesPanel } from "./ForcesPanel.tsx";
import { ActionForm } from "./ActionForm.tsx";
import { signatureOf, valuesOf } from "./draft.ts";

/** One section open: with three, the dock was as tall as the page and covered the graph. */
const OPEN_AT_FIRST: readonly string[] = ["Layout"];
const BODY = "gs-dock-body";

export interface DockProps {
  readonly studio: Studio;
  readonly state: StudioState;
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
      {studio.registry.actions.filter((action) => action.section === name).map((action) => (
        // WHY the id is in the key: three actions of one section take no parameters and so
        // share a signature, and React needs the key to be unique among the siblings.
        <ActionForm
          key={`${action.id} ${signatureOf(valuesOf(action, state))}`}
          studio={studio}
          action={action}
          state={state}
        />
      ))}
    </div>
  );
}

export function Dock(props: DockProps): ReactElement {
  const { studio, state, open, onToggle, bar } = props;
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
            {name === "Forces"
              ? <ForcesPanel studio={studio} state={state} bar={bar} />
              : <Actions studio={studio} state={state} name={name} />}
          </Section>
        ))}
      </div>
    </div>
  );
}
