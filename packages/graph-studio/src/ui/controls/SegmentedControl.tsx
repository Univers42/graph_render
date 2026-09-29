import type { ReactElement } from "react";

import { shortName } from "../names.ts";
import type { ControlProps } from "./props.ts";

/** A row of choices, for a list short enough that all of it should be in sight at once. */
export function SegmentedControl(props: ControlProps): ReactElement {
  const { spec, value, choices, disabled, onCommit } = props;
  const chosen = String(value);
  return (
    <div className="gs-row" role="group" aria-label={spec.title}>
      {choices.map((choice) => (
        <button
          key={choice}
          type="button"
          className="gs-btn"
          aria-pressed={choice === chosen}
          disabled={disabled}
          onClick={() => onCommit({ [spec.name]: choice })}
        >
          {shortName(choice)}
        </button>
      ))}
    </div>
  );
}
