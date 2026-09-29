import { useState, type ReactElement } from "react";

import type { ControlProps } from "./props.ts";

/** Runs on Enter or on blur, and only when the text is not what the action already holds. */
export function TextControl(props: ControlProps): ReactElement {
  const { spec, value, disabled, onCommit } = props;
  const [text, setText] = useState(() => String(value));
  const commit = (): void => {
    if (text !== String(value)) onCommit({ [spec.name]: text });
  };
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <input
        className="gs-input"
        type="text"
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
