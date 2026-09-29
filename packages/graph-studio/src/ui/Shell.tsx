/**
 * The chrome: floating panels over a canvas the studio does not own. It holds the whole
 * state in one subscription and knows only what is open — the console, the dock.
 */
import { useCallback, useRef, useState, type ReactElement } from "react";

import type { View } from "../../../graph-render/src/view.ts";
import type { Studio } from "../studio/studio.ts";
import { Console } from "./Console.tsx";
import { Dock } from "./Dock.tsx";
import { Hud } from "./Hud.tsx";
import { Inspector } from "./Inspector.tsx";
import { Legend } from "./Legend.tsx";
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
  const [consoleOpen, setConsole] = useState(false);
  const [dockOpen, setDock] = useState(true);
  const consoleInput = useRef<HTMLInputElement | null>(null);
  const searchInput = useRef<HTMLInputElement | null>(null);
  const focusConsole = useCallback(() => consoleInput.current?.focus(), []);
  const focusSearch = useCallback(() => searchInput.current?.focus(), []);
  useShortcuts({ studio, state, view, keys, consoleOpen, setConsole, focusConsole, focusSearch });
  return (
    <div className="gs-chrome" data-theme={state.settings.appearance.theme}>
      <div className="gs-left">
        <Search studio={studio} meta={state.meta} inputRef={searchInput} />
        <Inspector studio={studio} state={state} view={view} />
      </div>
      <Toast studio={studio} state={state} />
      <Dock studio={studio} state={state} open={dockOpen} onToggle={() => setDock((open) => !open)} />
      <div className="gs-bottom-left">
        <Legend state={state} />
        <Hud state={state} view={view} />
      </div>
      {consoleOpen && (
        <Console studio={studio} state={state} onClose={() => setConsole(false)} inputRef={consoleInput} />
      )}
    </div>
  );
}
