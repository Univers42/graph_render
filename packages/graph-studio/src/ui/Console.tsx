/** The console: the log, and the one line a command is typed on. */
import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent, type ReactElement } from "react";

import { type Completion, complete, ghost as ghostText } from "../console/complete.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { EMPTY_HISTORY, type History, type Step, pushLine, stepBack, stepForward } from "./history.ts";
import { LogView } from "./LogView.tsx";
import { shortName } from "./names.ts";

/** Candidates past this are counted, not listed: the line above the input has one row. */
const OFFERED = 12;
/** The input's border and padding: where its first character stands. */
const TEXT_AT = 7;

/**
 * Ponytail: the ghost sits after what is typed, and the offset is counted in `ch` — the
 * width of a zero — which is exact only in the monospace face the console sets on the
 * panel. With a proportional face the ghost drifts one character per narrow letter.
 * Its class is `gs-ghost` for the stylesheet; the placement here does not wait for it.
 */
function ghostAt(typed: number): CSSProperties {
  return {
    position: "absolute", top: 0, left: `calc(${TEXT_AT}px + ${typed}ch)`, lineHeight: "26px",
    color: "var(--gs-muted)", pointerEvents: "none", whiteSpace: "pre",
  };
}

export interface ConsoleProps {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly onClose: () => void;
}

interface Line {
  readonly text: string;
  readonly candidates: readonly string[];
  /** What Tab would add, shown in the line before Tab is pressed. */
  readonly ghost: string;
  readonly onChange: (text: string) => void;
  readonly onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
}

interface Keys {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly text: string;
  readonly onText: (text: string) => void;
  readonly onOffer: (done: Completion) => void;
  readonly onRun: () => void;
  readonly onWalk: (step: (history: History) => Step) => void;
}

function atEnd(event: KeyboardEvent<HTMLInputElement>): boolean {
  const input = event.currentTarget;
  return input.selectionStart === input.value.length && input.selectionEnd === input.value.length;
}

function onKey(event: KeyboardEvent<HTMLInputElement>, keys: Keys): void {
  const step = event.key === "ArrowUp" ? stepBack : event.key === "ArrowDown" ? stepForward : null;
  if (step !== null) event.preventDefault();
  if (step !== null) keys.onWalk(step);
  if (event.key === "Enter") keys.onRun();
  if (event.key === "Tab") {
    // Tab completes here and nowhere else: the focus never leaves the line being typed.
    event.preventDefault();
    keys.onOffer(complete(keys.text, keys.studio.registry, keys.state));
    return;
  }
  if (event.key !== "ArrowRight" || !atEnd(event)) return;
  // Right at the end of the line takes the ghost, as it does in a shell.
  const shown = ghostText(keys.text, keys.studio.registry, keys.state);
  if (shown === "") return;
  event.preventDefault();
  keys.onText(keys.text + shown);
}

/** The line, what it remembers and what Tab would offer: a hook, so the panel stays a panel. */
function useCommandLine(props: { readonly studio: Studio; readonly state: StudioState }): Line {
  const { studio, state } = props;
  const [text, setText] = useState("");
  const [candidates, setCandidates] = useState<readonly string[]>([]);
  const history = useRef<History>(EMPTY_HISTORY);
  const onText = (shown: string): void => {
    setText(shown);
    setCandidates([]);
  };
  const onOffer = (done: Completion): void => {
    setText(done.text);
    setCandidates(done.candidates);
  };
  const onRun = (): void => {
    if (text.trim() === "") return;
    void studio.run(text);
    history.current = pushLine(history.current, text);
    onText("");
  };
  const onWalk = (step: (history: History) => Step): void => {
    const next = step(history.current);
    history.current = next.history;
    setText(next.line ?? "");
  };
  const keys: Keys = { studio, state, text, onText, onOffer, onRun, onWalk };
  // The ghost follows the last character of the line: the word being completed is the
  // last one, so the completion always starts where the text ends.
  return {
    text,
    candidates,
    ghost: ghostText(text, studio.registry, state),
    onChange: setText,
    onKeyDown: (event) => onKey(event, keys),
  };
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
  const { studio, state, onClose } = props;
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
        <div style={{ position: "relative", flex: "1 1 auto" }}>
          <input
            className="gs-input"
            type="text"
            aria-label="Console command"
            value={line.text}
            ref={inputRef}
            onChange={(event) => line.onChange(event.target.value)}
            onKeyDown={line.onKeyDown}
          />
          <span className="gs-ghost" aria-hidden="true" style={ghostAt(line.text.length)}>{line.ghost}</span>
        </div>
      </div>
    </div>
  );
}
