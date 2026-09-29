/**
 * The chrome: floating panels over a canvas the studio does not own. It holds the whole
 * state in one subscription and knows only what is open — the console, the dock, and the
 * drawers the narrow layout turns the two panels into.
 */
import { useCallback, useEffect, useRef, useState, type ReactElement, type RefObject } from "react";

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

/** The stylesheet's own breakpoint: below it the panels are drawers, above it they float. */
const NARROW = "(max-width: 768px)";
/** The left panel's id, which is also its class, and the drawer's for the right one. */
const LEFT = "gs-left";
const DOCK = "gs-dock";

interface Drawers {
  readonly keys: Pick<EventTarget, "addEventListener" | "removeEventListener">;
  readonly consoleOpen: boolean;
  readonly busy: boolean;
  readonly leftOpen: boolean;
  readonly dockOpen: boolean;
  readonly setLeft: (open: boolean) => void;
  readonly setDock: (open: boolean) => void;
  readonly leftBtn: RefObject<HTMLButtonElement | null>;
  readonly dockBtn: RefObject<HTMLButtonElement | null>;
}

interface EscapeState {
  readonly consoleOpen: boolean;
  readonly busy: boolean;
  readonly leftOpen: boolean;
  readonly dockOpen: boolean;
  readonly closeLeft: () => void;
  readonly closeDock: () => void;
}

interface ToggleProps {
  readonly side: "left" | "right";
  readonly name: string;
  readonly target: string;
  readonly open: boolean;
  readonly box: RefObject<HTMLButtonElement | null>;
  readonly onToggle: () => void;
}

/** Was the press inside this drawer? The composed path is the real one, host and shadow and all. */
function within(name: string, path: readonly EventTarget[]): boolean {
  return path.some((one) => one instanceof HTMLElement && one.classList.contains(name));
}

/** Which drawer Escape closes: the one the press was in, else the one that is open. */
function escapeeOf(state: EscapeState, path: readonly EventTarget[]): (() => void) | null {
  const inLeft = within(LEFT, path);
  const inDock = within(DOCK, path);
  if (inLeft || (!inDock && state.leftOpen)) return state.closeLeft;
  if (inDock || state.dockOpen) return state.closeDock;
  return null;
}

/**
 * A drawer's open state, and the Escape that closes it with the focus handed back to the
 * toggle that opens it — the way a dialog hands focus to the button that opened it.
 *
 * WHY the listener reads its state from a ref and never re-registers: it has to take the
 * key before useShortcuts.ts, which would spend the same Escape on `view.clear`, and an
 * effect that re-registered when a drawer changed would be appended after it and too late.
 * Escape itself is not remapped: the console's line and the motor's cancel come first,
 * and above the breakpoint there is no drawer at all and the key keeps its old meaning.
 */
function useDrawers(drawers: Drawers): void {
  const { keys, consoleOpen, busy, leftOpen, dockOpen, setLeft, setDock, leftBtn, dockBtn } = drawers;
  const closeLeft = (): void => {
    setLeft(false);
    leftBtn.current?.focus();
  };
  const closeDock = (): void => {
    setDock(false);
    dockBtn.current?.focus();
  };
  const here: EscapeState = { consoleOpen, busy, leftOpen, dockOpen, closeLeft, closeDock };
  const live = useRef(here);
  useEffect(() => {
    live.current = here;
  });
  useEffect(() => {
    const onKey = (event: Event): void => {
      if (!(event instanceof KeyboardEvent) || event.key !== "Escape") return;
      const now = live.current;
      if (now.consoleOpen || now.busy || !window.matchMedia(NARROW).matches) return;
      const close = escapeeOf(now, event.composedPath());
      if (close === null) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      close();
    };
    keys.addEventListener("keydown", onKey);
    return () => keys.removeEventListener("keydown", onKey);
  }, [keys]);
}

/** A toggle for one drawer: fixed at its corner, so it stays reachable while closed. */
function Toggle(props: ToggleProps): ReactElement {
  const { side, name, target, open, box, onToggle } = props;
  return (
    <button
      type="button"
      ref={box}
      className={`gs-btn gs-toggle gs-toggle-${side}`}
      aria-expanded={open}
      aria-controls={target}
      onClick={onToggle}
    >
      {name}
    </button>
  );
}

export function Shell(props: ShellProps): ReactElement {
  const { studio, view, keys } = props;
  const state = useStudioState(studio);
  const [consoleOpen, setOpen] = useState(false);
  const [dockOpen, setDock] = useState(true);
  const [leftOpen, setLeft] = useState(false);
  const searchInput = useRef<HTMLInputElement | null>(null);
  const leftBtn = useRef<HTMLButtonElement | null>(null);
  const dockBtn = useRef<HTMLButtonElement | null>(null);
  const focusSearch = useCallback(() => searchInput.current?.focus(), []);
  // WHY the focus is handed back: the line that held it leaves the document with the
  // console, and a studio that listens on its own element would hear no key after that.
  const setConsole = useCallback((open: boolean) => {
    setOpen(open);
    if (!open && keys instanceof HTMLElement) keys.focus();
  }, [keys]);
  // WHY before useShortcuts: the drawer's listener has to be registered first to be heard first.
  useDrawers({ keys, consoleOpen, busy: state.busy.length > 0, leftOpen, dockOpen, setLeft, setDock, leftBtn, dockBtn });
  useShortcuts({ studio, state, keys, consoleOpen, setConsole, focusSearch });
  return (
    <div className="gs-chrome" data-theme={state.settings.appearance.theme}>
      <Toggle side="left" name="Panel" target={LEFT} open={leftOpen} box={leftBtn} onToggle={() => setLeft(!leftOpen)} />
      <Toggle side="right" name="Controls" target={DOCK} open={dockOpen} box={dockBtn} onToggle={() => setDock(!dockOpen)} />
      <div className={leftOpen ? `${LEFT} gs-open` : LEFT} id={LEFT}>
        <Search studio={studio} meta={state.meta} inputRef={searchInput} />
        <Inspector studio={studio} state={state} view={view} />
      </div>
      <Toast studio={studio} state={state} />
      <Dock studio={studio} state={state} open={dockOpen} onToggle={() => setDock(!dockOpen)} />
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
