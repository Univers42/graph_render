/** The menu a secondary click on a node opens: focus, pin, hide, copy the id. */
import { memo, useEffect, useRef, type KeyboardEvent, type ReactElement } from "react";

import type { Point } from "../../../graph-render/src/camera.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Studio } from "../studio/studio.ts";
import { type MenuItem, entriesFor, nextEntry } from "./nodeMenu.ts";

export interface MenuAt {
  readonly node: number;
  readonly at: Point;
}

/** The menu asks the view one thing: which nodes are pinned, so the entry can read Pin or Unpin. */
export type MenuView = Pick<View, "pinned">;

export interface NodeMenuProps {
  readonly studio: Studio;
  /** The ids of the graph as drawn, which is the one thing the menu copies out of the state. */
  readonly ids: readonly string[] | null;
  readonly view: MenuView;
  readonly menu: MenuAt | null;
  readonly onClose: () => void;
}

/**
 * WHY the id and not the index: an action takes a name and resolves it (`nodeNamed`,
 * `actions/view.ts`), which is what makes every gesture but Copy logged, typeable and checked
 * once. Copy is the exception and reads out of the state instead: what it wants is the text on
 * the clipboard, not a change to the drawing.
 */
function perform(item: MenuItem, props: NodeMenuProps, node: number): void {
  const { studio, ids } = props;
  if (item === "copy") {
    studio.copy(ids?.[node] ?? "");
    return;
  }
  const id = ids?.[node];
  if (id === undefined) return;
  if (item === "focus") void studio.dispatch("view.focus", { node: id });
  else if (item === "pin") void studio.dispatch("view.pin", { node: id });
  else if (item === "hide") void studio.dispatch("view.hide", { node: id });
}

function moveFocus(list: HTMLElement | null, event: KeyboardEvent): void {
  const buttons = Array.from(list?.querySelectorAll<HTMLButtonElement>("button") ?? []);
  const at = buttons.findIndex((button) => button === event.target);
  const to = nextEntry(at, event.key, buttons.length);
  if (to === at) return;
  event.preventDefault();
  buttons[to]?.focus();
}

/** Memoised: a menu that is closed draws nothing, and an open one reads four things. */
export const NodeMenu = memo(function NodeMenu(props: NodeMenuProps): ReactElement | null {
  const { menu, onClose, view } = props;
  const list = useRef<HTMLDivElement | null>(null);
  const opened = menu !== null;
  useEffect(() => {
    if (opened) list.current?.querySelector("button")?.focus();
  }, [opened, menu?.node]);
  if (menu === null) return null;
  const entries = entriesFor(view.pinned().includes(menu.node));
  const onKeyDown = (event: KeyboardEvent): void => {
    if (event.key !== "Escape") {
      moveFocus(list.current, event);
      return;
    }
    event.stopPropagation();
    onClose();
  };
  return (
    <div className="gs-menu-scrim" role="presentation" onPointerDown={onClose} onContextMenu={(event) => { event.preventDefault(); onClose(); }}>
      <div
        ref={list} className="gs-panel gs-menu" role="menu" tabIndex={-1} aria-label="Node actions"
        style={{ left: `min(${menu.at.x}px, calc(100% - 152px))`, top: `min(${menu.at.y}px, calc(100% - 148px))` }} onKeyDown={onKeyDown}
        onPointerDown={(event) => { event.stopPropagation(); }}
      >
        {entries.map((entry) => (
          <button
            key={entry.item} type="button" role="menuitem" className="gs-btn gs-menu-item"
            onClick={() => { perform(entry.item, props, menu.node); onClose(); }}
          >
            {entry.label}
          </button>
        ))}
      </div>
    </div>
  );
});
