import type { ReactElement } from "react";

import { shortName } from "../names.ts";
import type { ControlProps } from "./props.ts";

/** The native select: a long list of choices read once, with the keyboard the browser gives. */
export function SelectControl(props: ControlProps): ReactElement {
  const { spec, value, choices, disabled, onCommit } = props;
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <select
        className="gs-select"
        value={String(value)}
        disabled={disabled}
        onChange={(event) => onCommit({ [spec.name]: event.target.value })}
      >
        {choices.map((choice) => (
          <option key={choice} value={choice}>{shortName(choice)}</option>
        ))}
      </select>
    </label>
  );
}
