/**
 * The chrome: floating panels over a canvas the studio does not own. It holds the whole
 * state in one subscription and knows only what is open — the console, the dock.
 */
import { useCallback, useRef, useState, type ReactElement } from "react";

import { isLightTheme } from "../../../graph-render/src/look/themes.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Studio } from "../studio/studio.ts";
import { Console } from "./Console.tsx";
import { Dock } from "./Dock.tsx";
import { Hud } from "./Hud.tsx";
import { Inspector } from "./Inspector.tsx";
import { Legend } from "./Legend.tsx";
import { NavBar } from "./NavBar.tsx";
import { Search } from "./Search.tsx";
import { Toast } from "./Toast.tsx";
import { useShortcuts } from "./useShortcuts.ts";
import { useStudioState } from "./useStudio.ts";

export interface ShellProps {
  readonly studio: Studio;
  readonly view: Pick<View, "stats" | "on" | "focus" | "select">;
  /** Where key presses are listened for. */
  readonly keys: Pick<EventTarget, "addEventListener" | "removeEventListener">;
}

export function Shell(props: ShellProps): ReactElement {
  const { studio, view, keys } = props;
  const state = useStudioState(studio);
  const [consoleOpen, setOpen] = useState(false);
  const [dockOpen, setDock] = useState(true);
  const searchInput = useRef<HTMLInputElement | null>(null);
  const focusSearch = useCallback(() => searchInput.current?.focus(), []);
  // WHY the focus is handed back: the line that held it leaves the document with the
  // console, and a studio that listens on its own element would hear no key after that.
  const setConsole = useCallback((open: boolean) => {
    setOpen(open);
    if (!open && keys instanceof HTMLElement) keys.focus();
  }, [keys]);
  useShortcuts({ studio, state, keys, consoleOpen, setConsole, focusSearch });
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
      {consoleOpen && (
        <Console studio={studio} state={state} onClose={() => setConsole(false)} />
      )}
    </div>
  );
}
