import { useState, type ReactElement } from "react";

import type { ControlProps } from "./props.ts";

/** Runs on Enter or on blur, and only when the number typed is one the action can take. */
export function NumberControl(props: ControlProps): ReactElement {
  const { spec, value, disabled, onCommit } = props;
  const [text, setText] = useState(() => String(value));
  const commit = (): void => {
    const parsed = Number(text);
    const whole = spec.kind !== "number" ? Number.isInteger(parsed) : true;
    if (text.trim() === "" || !Number.isFinite(parsed) || !whole) {
      setText(String(value));
      return;
    }
    if (parsed !== Number(value)) onCommit({ [spec.name]: parsed });
  };
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <input
        className="gs-input"
        type="number"
        min={spec.min}
        max={spec.max}
        step={spec.step}
        value={text}
        disabled={disabled}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") commit();
        }}
        onBlur={commit}
      />
    </label>
  );
}
