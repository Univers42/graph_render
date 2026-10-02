/** The console: the log, and the one line a command is typed on. */
import { useEffect, useRef, useState, type KeyboardEvent, type ReactElement } from "react";

import { complete } from "../console/complete.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { EMPTY_HISTORY, type History, type Step, pushLine, stepBack, stepForward } from "./history.ts";
import { LogView } from "./LogView.tsx";
import { shortName } from "./names.ts";
import { useStudioState } from "./useStudio.ts";

/** Candidates past this are counted, not listed: the line above the input has one row. */
const OFFERED = 12;

export interface ConsoleProps {
  readonly studio: Studio;
  readonly onClose: () => void;
}

interface Line {
  readonly text: string;
  readonly candidates: readonly string[];
  readonly onChange: (text: string) => void;
  readonly onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
}

/** The line, what it remembers and what Tab would offer: a hook, so the panel stays a panel. */
function useCommandLine(props: { readonly studio: Studio; readonly state: StudioState }): Line {
  const { studio, state } = props;
  const [text, setText] = useState("");
  const [candidates, setCandidates] = useState<readonly string[]>([]);
  const history = useRef<History>(EMPTY_HISTORY);
  const run = (): void => {
    if (text.trim() === "") return;
    void studio.run(text);
    history.current = pushLine(history.current, text);
    setText("");
    setCandidates([]);
  };
  const walk = (step: (history: History) => Step): void => {
    const next = step(history.current);
    history.current = next.history;
    setText(next.line ?? "");
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>): void => {
    const step = event.key === "ArrowUp" ? stepBack : event.key === "ArrowDown" ? stepForward : null;
    if (step !== null) event.preventDefault();
    if (step !== null) walk(step);
    if (event.key === "Enter") run();
    if (event.key !== "Tab") return;
    // Tab completes here and nowhere else: the focus never leaves the line being typed.
    event.preventDefault();
    const done = complete(text, studio.registry, state);
    setText(done.text);
    setCandidates(done.candidates);
  };
  return { text, candidates, onChange: setText, onKeyDown };
}

function Offered(props: { readonly candidates: readonly string[] }): ReactElement | null {
  const { candidates } = props;
  if (candidates.length === 0) return null;
  const shown = candidates.slice(0, OFFERED);
  const rest = candidates.length - shown.length;
  return (
    <div className="gs-candidates">
      {shown.map((one) => <span className="gs-candidate" key={one}>{shortName(one)}</span>)}
      {rest > 0 && <span className="gs-more">{`+${rest}`}</span>}
    </div>
  );
}

export function Console(props: ConsoleProps): ReactElement {
  const { studio, onClose } = props;
  // WHY it subscribes itself: completion reads the whole state, and the console is only
  // mounted while it is open, so nothing pays for it while it is shut.
  const state = useStudioState(studio);
  const line = useCommandLine({ studio, state });
  const boxRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  // WHY here and not where the key is handled: the line is not in the document until the
  // console has rendered, and whoever opened it with a click asked for the line as well.
  useEffect(() => inputRef.current?.focus(), []);
  return (
    <div className="gs-panel gs-console">
      <div className="gs-head">
        <span className="gs-head-name">Console</span>
        <button
          type="button"
          className="gs-btn"
          aria-label="Clear the log"
          onClick={() => void studio.dispatch("console.clear")}
        >
          Clear
        </button>
        <button type="button" className="gs-btn" aria-label="Close the console" onClick={onClose}>Close</button>
      </div>
      <LogView entries={state.log} boxRef={boxRef} />
      <Offered candidates={line.candidates} />
      <div className="gs-console-form">
        <input
          className="gs-input"
          type="text"
          aria-label="Console command"
          value={line.text}
          ref={inputRef}
          onChange={(event) => line.onChange(event.target.value)}
          onKeyDown={line.onKeyDown}
        />
      </div>
    </div>
  );
}
