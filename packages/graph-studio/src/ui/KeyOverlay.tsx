/**
 * The key bindings, as a dialog over the chrome. It draws inline and is positioned by CSS:
 * the chrome lives in a shadow root, where a portal to `document.body` would leave its styles.
 */
import type { ReactElement } from "react";

import { studioActions } from "../actions/all.ts";
import { keyBindings } from "./keymap.ts";

export interface KeyOverlayProps {
  readonly open: boolean;
  readonly onClose: () => void;
}

export function KeyOverlay({ open, onClose }: KeyOverlayProps): ReactElement | null {
  if (!open) return null;
  const titles = new Map(studioActions().map((action) => [action.id, action.title]));
  const rows = keyBindings(titles);
  return (
    <div className="gs-keymap gs-panel" role="dialog" aria-label="Key bindings" data-keymap>
      <header className="gs-keymap-head">
        <h2>Key bindings</h2>
        <button type="button" className="gs-keymap-close" aria-label="Close" onClick={onClose}>×</button>
      </header>
      <div className="gs-keymap-scroll">
        <table>
          <thead><tr><th>Key</th><th>Action</th><th>Description</th></tr></thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key} data-key={row.key} data-action={row.id}>
                <td><kbd>{row.key}</kbd></td><td>{row.id}</td><td>{row.title}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
