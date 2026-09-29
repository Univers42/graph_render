/** The menu a secondary click on a node opens: focus, pin, hide, copy the id. */
import { useEffect, useRef, type KeyboardEvent, type ReactElement } from "react";

import type { Point } from "../../../graph-render/src/camera.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { type MenuItem, entriesFor, nextEntry } from "./nodeMenu.ts";

export interface MenuAt {
  readonly node: number;
  readonly at: Point;
}

export type MenuView = Pick<View, "focus" | "hide" | "togglePin" | "pinned">;

export interface NodeMenuProps {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly view: MenuView;
  readonly menu: MenuAt | null;
  readonly onClose: () => void;
}

function perform(item: MenuItem, props: NodeMenuProps, node: number): void {
  const { studio, state, view } = props;
  if (item === "focus") view.focus(node);
  else if (item === "pin") view.togglePin(node);
  else if (item === "hide") view.hide([node]);
  else studio.copy(state.meta?.ids[node] ?? "");
}

function moveFocus(list: HTMLElement | null, event: KeyboardEvent): void {
  const buttons = Array.from(list?.querySelectorAll<HTMLButtonElement>("button") ?? []);
  const at = buttons.findIndex((button) => button === event.target);
  const to = nextEntry(at, event.key, buttons.length);
  if (to === at) return;
  event.preventDefault();
  buttons[to]?.focus();
}

export function NodeMenu(props: NodeMenuProps): ReactElement | null {
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
}
