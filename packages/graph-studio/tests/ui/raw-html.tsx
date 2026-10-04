/**
 * The break of rows `host-api-escape` and `lint` (`docs/contract/host-api.md`, verdict 5): every
 * markup sink the lint bans, in the one file `app/eslint.config.js` ignores.
 * - `scripts/studio.sh lint` lints this file alone and expects each ban to fire.
 * - `HOST_API_ESCAPE_BREAK=1` renders previews through `RawPreview`, and `host-escape.test.tsx`
 *   must then fail.
 */
import { createElement, type ReactElement } from "react";

import type { NodePreview } from "../../src/host/contract.ts";

export function RawPreview(props: { readonly preview: NodePreview }): ReactElement {
  const { title, text = "", icon = "" } = props.preview;
  return <div className="gs-preview" dangerouslySetInnerHTML={{ __html: `${icon}${title}${text}` }} />;
}

/** Never called: one line per sink the lint must name. */
export function everySink(element: Element, html: string): ReactElement {
  element.innerHTML = html;
  element.outerHTML = html;
  element.insertAdjacentHTML("beforeend", html);
  return createElement("div", { dangerouslySetInnerHTML: { __html: html } });
}
