/** Finding a node by name: a line of text, and the first eight labels that hold it. */
import { useState, type KeyboardEvent, type ReactElement, type RefObject } from "react";

import type { GraphMeta } from "../source/meta.ts";
import type { Studio } from "../studio/studio.ts";
import { matchesOf } from "./matches.ts";

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

export function Search(props: SearchProps): ReactElement {
  const { studio, meta, inputRef } = props;
  const [text, setText] = useState("");
  const labels = meta?.labels ?? [];
  const found = matchesOf(labels, text);
  const go = (node: number): void => {
    // The id, not the name: labels are shared between nodes and ids are not.
    const id = meta?.ids[node];
    if (id === undefined) return;
    void studio.dispatch("view.focus", { node: id });
    setText("");
  };
  const key = (event: KeyboardEvent<HTMLInputElement>): void => {
    if (event.key === "Escape") setText("");
    const first = event.key === "Enter" ? found[0] : undefined;
    if (first !== undefined) go(first);
  };
  return (
    <div className="gs-panel gs-search">
      <input
        className="gs-input"
        type="search"
        aria-label="Search nodes"
        placeholder="Search ( / )"
        value={text}
        ref={inputRef}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={key}
      />
      <Results found={found} labels={labels} go={go} />
    </div>
  );
}
