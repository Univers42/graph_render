/** Finding a node by name: a line of text, the labels that hold it, and a camera on them. */
import type { KeyboardEvent, ReactElement, RefObject } from "react";

import type { GraphMeta } from "../source/meta.ts";
import type { Studio } from "../studio/studio.ts";
import { allMatchesOf } from "./matches.ts";
import { useStudioState } from "./useStudio.ts";

/** How many labels the box lists. The mask and the Enter key see the whole of them. */
const RESULTS_SHOWN = 8;

export interface SearchProps {
  readonly studio: Studio;
  /** `null` before anything is drawn: the line is still there, with nothing to offer. */
  readonly meta: GraphMeta | null;
  readonly inputRef: RefObject<HTMLInputElement | null>;
}

interface ResultsProps {
  readonly found: readonly number[];
  readonly labels: readonly string[];
  readonly go: (node: number) => void;
}

interface SearchActions {
  readonly search: (text: string) => void;
  readonly go: (node: number) => void;
  readonly key: (event: KeyboardEvent<HTMLInputElement>) => void;
}

function Results(props: ResultsProps): ReactElement | null {
  const { found, labels, go } = props;
  if (found.length === 0) return null;
  return (
    <div className="gs-results">
      {found.map((node) => (
        <button key={node} type="button" className="gs-btn gs-result" onClick={() => go(node)}>
          {labels[node] ?? ""}
        </button>
      ))}
    </div>
  );
}

function actionsOf(studio: Studio, meta: GraphMeta | null, found: readonly number[]): SearchActions {
  // Every control here is an action: the console line a reader types must be the same one.
  const search = (text: string): void => void studio.dispatch("search", { text });
  const go = (node: number): void => {
    // The id, not the name: labels are shared between nodes and ids are not.
    const id = meta?.ids[node];
    if (id === undefined) return;
    search("");
    void studio.dispatch("view.focus", { node: id });
  };
  const key = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Escape") search("");
    const first = event.key === "Enter" ? found[0] : undefined;
    if (first !== undefined) go(first);
  };
  return { search, go, key };
}

export function Search(props: SearchProps): ReactElement {
  const { studio, meta, inputRef } = props;
  const state = useStudioState(studio);
  const labels = meta?.labels ?? [];
  // The whole ranked list, not the eight on screen: the mask the search action asks for is
  // made of every one of them, and a box that offered a set the mask disagreed with would
  // be a second answer to the same question.
  const found = allMatchesOf(labels, state.settings.filter.text);
  const { search, go, key } = actionsOf(studio, meta, found);
  return (
    <div className="gs-panel gs-search">
      <input
        className="gs-input"
        type="search"
        aria-label="Search nodes"
        placeholder="Search ( / )"
        value={state.settings.filter.text}
        ref={inputRef}
        onChange={(event) => search(event.target.value)}
        onKeyDown={key}
      />
      <Results found={found.slice(0, RESULTS_SHOWN)} labels={labels} go={go} />
      <button
        type="button"
        className="gs-btn"
        aria-label="Fit to results"
        title={`Fit the camera to the ${found.length} results`}
        disabled={found.length === 0}
        onClick={() => void studio.dispatch("search.fit")}
      >
        fit to results
      </button>
    </div>
  );
}
