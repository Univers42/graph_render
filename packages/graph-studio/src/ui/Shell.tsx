/**
 * The chrome: floating panels over a canvas the studio does not own. It holds the whole
 * state in one subscription and knows only what is open — the console, the dock.
 */
import { useCallback, useEffect, useRef, useState, type ReactElement } from "react";

import { isLightTheme } from "../../../graph-render/src/look/themes.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Studio } from "../studio/studio.ts";
import { KeyOverlay } from "./KeyOverlay.tsx";
import { Console } from "./Console.tsx";
import { Dock } from "./Dock.tsx";
import { Hud } from "./Hud.tsx";
import { Inspector } from "./Inspector.tsx";
import { Legend } from "./Legend.tsx";
import { NavBar } from "./NavBar.tsx";
import { type MenuAt, NodeMenu, type MenuView } from "./NodeMenu.tsx";
import { Search } from "./Search.tsx";
import { Toast } from "./Toast.tsx";
import { useShortcuts } from "./useShortcuts.ts";
import { useStudioState } from "./useStudio.ts";

export interface ShellProps {
  readonly studio: Studio;
  readonly view: Pick<View, "stats" | "on" | "focus" | "select" | "camera" | "position"> & MenuView;
  /** Where key presses are listened for. */
  readonly keys: Pick<EventTarget, "addEventListener" | "removeEventListener">;
}

interface NodeMenuOpening {
  readonly view: Pick<ShellProps["view"], "on" | "camera" | "position">;
  readonly keys: ShellProps["keys"];
  readonly selected: number;
}

/**
 * A secondary click opens the menu at the pointer; the menu key opens it at the selected node.
 * Returns where the menu is open, or null, and what closes it.
 */
function useNodeMenu(opening: NodeMenuOpening): readonly [MenuAt | null, () => void] {
  const { view, keys, selected } = opening;
  const [menu, open] = useState<MenuAt | null>(null);
  const close = useCallback(() => open(null), []);
  useEffect(() => view.on("context", ({ node, at }) => open(node >= 0 ? { node, at } : null)), [view, open]);
  useEffect(() => {
    const onKey = (event: Event): void => {
      if (!(event instanceof KeyboardEvent) || selected < 0) return;
      if (event.key !== "ContextMenu" && !(event.key === "F10" && event.shiftKey)) return;
      event.preventDefault();
      const { x, y } = view.position(selected);
      const camera = view.camera();
      open({ node: selected, at: { x: x * camera.scale + camera.x, y: y * camera.scale + camera.y } });
    };
    keys.addEventListener("keydown", onKey);
    return () => keys.removeEventListener("keydown", onKey);
  }, [view, keys, selected]);
  return [menu, close];
}

export function Shell(props: ShellProps): ReactElement {
  const { studio, view, keys } = props;
  const state = useStudioState(studio);
  const [consoleOpen, setOpen] = useState(false);
  const [helpOpen, setHelp] = useState(false);
  const toggleHelp = useCallback(() => setHelp((open) => !open), []);
  const [dockOpen, setDock] = useState(true);
  const searchInput = useRef<HTMLInputElement | null>(null);
  const focusSearch = useCallback(() => searchInput.current?.focus(), []);
  // WHY the focus is handed back: the line that held it leaves the document with the
  // console, and a studio that listens on its own element would hear no key after that.
  const setConsole = useCallback((open: boolean) => {
    setOpen(open);
    if (!open && keys instanceof HTMLElement) keys.focus();
  }, [keys]);
  const [menu, closeMenu] = useNodeMenu({ view, keys, selected: state.selected });
  useShortcuts({ studio, state, keys, consoleOpen, setConsole, focusSearch, toggleHelp, helpOpen });
  return (
    <div className="gs-chrome" data-theme={isLightTheme(state.settings.appearance.theme) ? "light" : "dark"}>
      <div className="gs-left">
        <Search studio={studio} meta={state.meta} inputRef={searchInput} />
        <Inspector studio={studio} state={state} view={view} />
      </div>
      <Toast studio={studio} state={state} />
      <Dock studio={studio} state={state} open={dockOpen} onToggle={() => setDock((open) => !open)} />
      <div className="gs-bottom-left">
        <Legend state={state} />
        <Hud state={state} view={view} />
        <NavBar studio={studio} />
      </div>
      <KeyOverlay open={helpOpen} onClose={() => setHelp(false)} />
      <NodeMenu studio={studio} state={state} view={view} menu={menu} onClose={closeMenu} />
      {consoleOpen && (
        <Console studio={studio} state={state} onClose={() => setConsole(false)} />
      )}
    </div>
  );
}
