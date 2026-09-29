import type { ChangeEvent, ReactElement } from "react";

import type { ControlProps } from "./props.ts";

function picked(event: ChangeEvent<HTMLInputElement>): File | null {
  return event.target.files?.[0] ?? null;
}

/**
 * WHY the file's own name travels with its text: `source.document` takes both a name and a
 * text, and a document the reader picked is called what the file is called. An action with
 * no `name` parameter (`recipe.replay`) has nowhere to put it, so it is left out.
 */
function patchOf(props: ControlProps, file: File, text: string): Readonly<Record<string, string>> {
  return props.named ? { [props.spec.name]: text, name: file.name } : { [props.spec.name]: text };
}

export function FileControl(props: ControlProps): ReactElement {
  const { spec, disabled, onCommit } = props;
  const read = (event: ChangeEvent<HTMLInputElement>): void => {
    const file = picked(event);
    if (file === null) return;
    void file.text().then((text) => onCommit(patchOf(props, file, text)));
  };
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <input className="gs-input" type="file" disabled={disabled} onChange={read} />
    </label>
  );
}
