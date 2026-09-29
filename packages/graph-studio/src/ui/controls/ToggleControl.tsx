import type { ReactElement } from "react";

import type { ControlProps } from "./props.ts";

/** A switch: the value is the setting, so there is nothing to confirm. */
export function ToggleControl(props: ControlProps): ReactElement {
  const { spec, value, disabled, onCommit } = props;
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <input
        className="gs-check"
        type="checkbox"
        role="switch"
        checked={value === true}
        disabled={disabled}
        onChange={(event) => onCommit({ [spec.name]: event.target.checked })}
      />
    </label>
  );
}
