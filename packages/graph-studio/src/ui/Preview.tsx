/**
 * What a host said about a node, as the hover card and the inspector both show it.
 *
 * WHY text children only: every string here came from the host or from the document, and React
 * escapes a text child. No attribute is built from them, so no `href`, `src` or handler can
 * appear whatever they hold (`host-api-escape`, verdict 5).
 */
import type { ReactElement } from "react";

import type { NodePreview } from "../host/contract.ts";

export function PreviewBody(props: { readonly preview: NodePreview }): ReactElement {
  const { title, text, icon } = props.preview;
  return (
    <div className="gs-preview">
      <div className="gs-preview-head">
        {icon !== undefined && <span className="gs-preview-icon">{icon}</span>}
        <span className="gs-preview-title">{title}</span>
      </div>
      {text !== undefined && <p className="gs-preview-text">{text}</p>}
    </div>
  );
}
