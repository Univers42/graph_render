/**
 * The chrome: floating panels over a canvas the studio does not own. It reads the slices
 * the panels draw and knows only what is open — the console, the dock.
 */
import { useCallback, useEffect, useRef, useState, type ReactElement } from "react";

import { isLightTheme } from "../../../graph-render/src/look/themes.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { LiveBridge } from "../motor/bridge.ts";
import type { AnalysisReport } from "../motor/protocol.ts";
import type { GraphMeta } from "../source/meta.ts";
import type { ShownError } from "../state/errors.ts";
import type { RunSummary, Running } from "../state/model.ts";
import type { Settings } from "../state/settings.ts";
import type { Studio } from "../studio/studio.ts";
import { KeyOverlay } from "./KeyOverlay.tsx";
import { Console } from "./Console.tsx";
import { Dock } from "./Dock.tsx";
import { Hud } from "./Hud.tsx";
import { Inspector } from "./Inspector.tsx";
import { Legend } from "./Legend.tsx";
import { NavBar } from "./NavBar.tsx";
import { type MenuAt, NodeMenu, type MenuView } from "./NodeMenu.tsx";
import { ProgressBar } from "./ProgressBar.tsx";
import { Search } from "./Search.tsx";
import { Toast } from "./Toast.tsx";
import { useShortcuts } from "./useShortcuts.ts";
import { useStudioSelector } from "./useStudio.ts";

export interface ShellProps {
  readonly studio: Studio;
  readonly view: Pick<View, "stats" | "on" | "focus" | "select" | "camera" | "position"> & MenuView;
  /** Where key presses are listened for. */
  readonly keys: Pick<EventTarget, "addEventListener" | "removeEventListener">;
  /** The live loop's own store: read at its own rate, never with the rest of the chrome. */
  readonly bar: Pick<LiveBridge, "bar" | "onBar">;
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

/** The slices the panels below draw. This list is the whole of what re-renders the chrome. */
interface Slices {
  readonly settings: Settings;
  readonly meta: GraphMeta | null;
  readonly analysis: AnalysisReport | null;
  readonly selection: readonly number[];
  readonly run: RunSummary | null;
  readonly busy: readonly Running[];
  readonly failure: ShownError | null;
  readonly selected: number;
}

/**
 * WHY slices and not the whole state: the shell re-renders on a change to any of these and
 * on nothing else, and every panel it hands them to is memoised, so a field nobody draws
 * costs the studio nothing at all.
 */
function useSlices(studio: Studio): Slices {
  return {
    settings: useStudioSelector(studio, (state) => state.settings),
    meta: useStudioSelector(studio, (state) => state.meta),
    analysis: useStudioSelector(studio, (state) => state.analysis),
    selection: useStudioSelector(studio, (state) => state.selection),
    run: useStudioSelector(studio, (state) => state.run),
    busy: useStudioSelector(studio, (state) => state.busy),
    failure: useStudioSelector(studio, (state) => state.error),
    selected: useStudioSelector(studio, (state) => state.selected),
  };
}

export function Shell(props: ShellProps): ReactElement {
  const { studio, view, keys, bar } = props;
  const { settings, meta, analysis, selection, run, busy, failure, selected } = useSlices(studio);
  const [consoleOpen, setOpen] = useState(false);
  const [helpOpen, setHelp] = useState(false);
  const toggleHelp = useCallback(() => setHelp((open) => !open), []);
  const [dockOpen, setDock] = useState(true);
  const toggleDock = useCallback(() => setDock((open) => !open), []);
  const searchInput = useRef<HTMLInputElement | null>(null);
  const focusSearch = useCallback(() => searchInput.current?.focus(), []);
  // WHY the focus is handed back: the line that held it leaves the document with the
  // console, and a studio that listens on its own element would hear no key after that.
  const setConsole = useCallback((open: boolean) => {
    setOpen(open);
    if (!open && keys instanceof HTMLElement) keys.focus();
  }, [keys]);
  const [menu, closeMenu] = useNodeMenu({ view, keys, selected });
  useShortcuts({ studio, busy: busy.length > 0, keys, consoleOpen, setConsole, focusSearch, toggleHelp, helpOpen });
  const closeConsole = useCallback(() => setConsole(false), [setConsole]);
  return (
    <div className="gs-chrome" data-theme={isLightTheme(settings.appearance.theme) ? "light" : "dark"}>
      <ProgressBar bar={bar.bar} onBar={bar.onBar} />
      <div className="gs-left">
        <Search studio={studio} meta={meta} text={settings.filter.text} inputRef={searchInput} />
        <Inspector studio={studio} meta={meta} selected={selected} analysis={analysis} selection={selection} view={view} />
      </div>
      <Toast studio={studio} busy={busy} error={failure} />
      <Dock studio={studio} bar={bar} open={dockOpen} onToggle={toggleDock} />
      <div className="gs-bottom-left">
        <Legend meta={meta} settings={settings} analysis={analysis} />
        <Hud run={run} view={view} />
        <NavBar studio={studio} />
      </div>
      <KeyOverlay open={helpOpen} onClose={toggleHelp} />
      <NodeMenu studio={studio} ids={meta?.ids ?? null} view={view} menu={menu} onClose={closeMenu} />
      {consoleOpen && <Console studio={studio} onClose={closeConsole} />}
    </div>
  );
}
