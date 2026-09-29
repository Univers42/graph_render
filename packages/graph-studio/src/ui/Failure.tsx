/** What a failure says, in one place: the toast raises it, the log repeats it. */
import type { ReactElement } from "react";

import type { ShownError } from "../state/errors.ts";

export interface FailureProps {
  readonly error: ShownError;
  /** The toast announces itself; the log is already a live region and must not shout twice. */
  readonly announced: boolean;
  readonly onDismiss?: () => void;
}

export function Failure(props: FailureProps): ReactElement {
  const { error, announced, onDismiss } = props;
  return (
    <div className="gs-alert" role={announced ? "alert" : undefined}>
      <div className="gs-alert-head">
        <span className="gs-alert-title">{error.title}</span>
        {error.code !== null && <span className="gs-value">{error.code}</span>}
        {onDismiss !== undefined && (
          <button type="button" className="gs-btn" aria-label="Dismiss the error" onClick={onDismiss}>Dismiss</button>
        )}
      </div>
      <div className="gs-detail">{error.detail}</div>
      <div className="gs-hint">{error.hint}</div>
    </div>
  );
}
