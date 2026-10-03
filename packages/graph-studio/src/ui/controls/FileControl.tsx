import { type ChangeEvent, type ReactElement, useState } from "react";

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

/**
 * Why a file is refused before it is read, or null. Its size is in bytes, never fewer than the
 * characters it decodes to, so a file this lets through can still be refused by `resolve`.
 */
function sizeRefusal(props: ControlProps, file: File): string | null {
  const most = props.spec.max;
  if (most === undefined || file.size <= most) return null;
  return `${file.name} is ${file.size} bytes; at most ${most} are read`;
}

export function FileControl(props: ControlProps): ReactElement {
  const { spec, disabled, onCommit } = props;
  const [refused, setRefused] = useState<string | null>(null);
  const read = (event: ChangeEvent<HTMLInputElement>): void => {
    const file = picked(event);
    if (file === null) return;
    const reason = sizeRefusal(props, file);
    setRefused(reason);
    if (reason === null) void file.text().then((text) => onCommit(patchOf(props, file, text)));
  };
  return (
    <label className="gs-field">
      <span className="gs-field-label">{spec.title}</span>
      <input className="gs-input" type="file" disabled={disabled} onChange={read} />
      {refused === null ? null : <span role="alert" className="gs-hint">{refused}</span>}
    </label>
  );
}
