/** What the studio did, oldest first, each entry as the line that would ask for it again. */
import { useEffect, type ReactElement, type RefObject } from "react";

import type { LogEntry } from "../state/model.ts";
import { Failure } from "./Failure.tsx";
import { digest8, ms } from "./names.ts";

interface EntryProps {
  readonly entry: LogEntry;
}

function Entry(props: EntryProps): ReactElement {
  const { entry } = props;
  const failed = !entry.ok;
  return (
    <div className={failed ? "gs-entry gs-failed" : "gs-entry"}>
      <div className="gs-line">
        <span className="gs-cmd">{`> ${entry.command}`}</span>
        <span className="gs-value">{ms(entry.ms)}</span>
      </div>
      <div>{entry.message}</div>
      {entry.digest !== null && <div className="gs-value">{digest8(entry.digest)}</div>}
      {entry.notes.map((note, at) => <div className="gs-note" key={at}>{note}</div>)}
      {entry.error !== null && <Failure error={entry.error} announced={false} />}
    </div>
  );
}

export interface LogViewProps {
  readonly entries: readonly LogEntry[];
  readonly boxRef: RefObject<HTMLDivElement | null>;
}

export function LogView(props: LogViewProps): ReactElement {
  const { entries, boxRef } = props;
  // WHY the scroll is here and not in a scroll handler: a live region is read from its end,
  // and the newest entry is the only one anybody came back for.
  useEffect(() => {
    const box = boxRef.current;
    if (box !== null) box.scrollTop = box.scrollHeight;
  }, [entries.length, boxRef]);
  return (
    <div className="gs-log" role="log" aria-live="polite" aria-label="What the studio did" ref={boxRef}>
      {entries.map((entry) => <Entry key={entry.seq} entry={entry} />)}
    </div>
  );
}
