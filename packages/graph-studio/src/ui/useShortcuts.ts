/**
 * The keys, and nothing else. A shortcut is a function of the key and of who holds the
 * keyboard, so what it does can be read without a browser.
 */
import { useEffect } from "react";

import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";
import { type Held, type NavKey, navKeyOf } from "./navKeys.ts";

export type Shortcut = "console" | "search" | "help" | "escape" | null;

const TYPING = new Set(["INPUT", "SELECT", "TEXTAREA"]);

/** What a key press says about itself; a `KeyboardEvent` is one. */
export type Pressed = Pick<KeyboardEvent, "key" | "code" | "ctrlKey" | "metaKey" | "altKey">;

/** The keys the chrome itself owns; the camera keys are actions (navKeys.ts). */
export function chromeOf(pressed: Pressed, typing: boolean): Shortcut {
  if (pressed.ctrlKey || pressed.metaKey || pressed.altKey) return null;
  // WHY the code too: `key` is what the layout prints, and the key left of 1 prints º or ²
  // on a keyboard that has no backquote there.
  if (pressed.key === "`" || pressed.code === "Backquote") return "console";
  if (pressed.key === "/") return typing ? null : "search";
  if (pressed.key === "?") return typing ? null : "help";
  if (pressed.key === "Escape") return "escape";
  return null;
}

/** The camera key for this press, or null; a field that holds the keyboard takes it away. */
export function navigationOf(pressed: Pressed, typing: boolean): NavKey | null {
  if (typing) return null;
  return navKeyOf(pressed.key, heldOf(pressed));
}

function heldOf(pressed: Pressed): Held {
  return { ctrlKey: pressed.ctrlKey, metaKey: pressed.metaKey, altKey: pressed.altKey };
}

/**
 * WHY the composed path and not the target: the chrome lives in a shadow root, where
 * `event.target` is the host element and every key looks as if nothing was being typed in.
 */
function typingAt(path: readonly EventTarget[]): boolean {
  const first = path[0];
  if (!(first instanceof HTMLElement)) return false;
  return TYPING.has(first.tagName) || first.isContentEditable;
}

export interface ShortcutProps {
  readonly studio: Studio;
  readonly state: StudioState;
  readonly keys: Pick<EventTarget, "addEventListener" | "removeEventListener">;
  readonly consoleOpen: boolean;
  readonly setConsole: (open: boolean) => void;
  readonly focusSearch: () => void;
  readonly toggleHelp: () => void;
  readonly helpOpen: boolean;
}

export function useShortcuts(props: ShortcutProps): void {
  const { studio, state, keys, consoleOpen, setConsole, focusSearch, toggleHelp, helpOpen } = props;
  const busy = state.busy.length > 0;
  useEffect(() => {
    const onKey = (event: Event): void => {
      if (!(event instanceof KeyboardEvent)) return;
      const typing = typingAt(event.composedPath());
      // The way out of what is open comes first: Escape is never a camera key here.
      if (event.key === "Escape" && consoleOpen) {
        event.preventDefault();
        setConsole(false);
        return;
      }
      if (event.key === "Escape" && helpOpen) {
        event.preventDefault();
        toggleHelp();
        return;
      }
      if (busy && event.key === "Escape") {
        event.preventDefault();
        void studio.dispatch("view.cancel");
        return;
      }
      const chrome = chromeOf(event, typing);
      if (chrome !== null) {
        // The browser's own meaning is never wanted here: `/` opens quick find, `f` types.
        event.preventDefault();
        if (chrome === "console") setConsole(!consoleOpen);
        else if (chrome === "search") focusSearch();
        else if (chrome === "help") toggleHelp();
        else void studio.dispatch("view.clear");
        return;
      }
      const nav = navigationOf(event, typing);
      if (nav === null) return;
      // Arrows scroll the page and `+` types into a field: the studio is the whole page here,
      // but a host that embeds it beside its own inputs keeps them.
      event.preventDefault();
      void studio.dispatch(nav.id, { ...nav.args });
    };
    keys.addEventListener("keydown", onKey);
    return () => keys.removeEventListener("keydown", onKey);
  }, [studio, keys, consoleOpen, busy, setConsole, focusSearch, toggleHelp, helpOpen]);
}
