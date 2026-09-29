import type { ReactElement } from "react";

import { shortName } from "../names.ts";
import type { ControlProps } from "./props.ts";

/** A column of choices, for a list long enough that scrolling past a select would be worse. */
export function ListControl(props: ControlProps): ReactElement {
  const { spec, value, choices, disabled, onCommit } = props;
  const chosen = String(value);
  return (
    <div className="gs-list" role="group" aria-label={spec.title}>
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
