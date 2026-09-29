import type { ReactElement } from "react";

import { sig3 } from "../names.ts";
import type { ControlProps } from "./props.ts";

/**
 * The thumb follows the pointer and the action runs when the pointer is let go: a run per
 * pixel of drag would ask the motor for the same layout a hundred times.
 */
export function SliderControl(props: ControlProps): ReactElement {
  const { spec, value, disabled, onDraft, onCommit } = props;
  const shown = Number(value);
  const commit = (): void => onCommit({ [spec.name]: shown });
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <span className="gs-row">
        <input
          className="gs-range"
          type="range"
          min={spec.min}
          max={spec.max}
          step={spec.step}
          value={String(value)}
          disabled={disabled}
          onChange={(event) => onDraft({ [spec.name]: Number(event.target.value) })}
          onPointerUp={commit}
          onKeyUp={commit}
          onBlur={commit}
        />
        <span className="gs-value">{sig3(shown)}</span>
      </span>
    </label>
  );
}
