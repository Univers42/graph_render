/**
 * The keys, and nothing else. A shortcut is a function of the key and of who holds the
 * keyboard, so what it does can be read without a browser.
 */
import { useEffect } from "react";

import type { View } from "../../../graph-render/src/view.ts";
import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";

export type Shortcut = "console" | "search" | "fit" | "escape" | null;

const TYPING = new Set(["INPUT", "SELECT", "TEXTAREA"]);

export function shortcutOf(key: string, typing: boolean): Shortcut {
  if (key === "Backquote") return "console";
  if (key === "/") return typing ? null : "search";
  if (key === "f") return typing ? null : "fit";
  if (key === "Escape") return "escape";
  return null;
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
  readonly view: Pick<View, "select">;
  readonly keys: Pick<EventTarget, "addEventListener" | "removeEventListener">;
  readonly consoleOpen: boolean;
  readonly setConsole: (open: boolean) => void;
  readonly focusConsole: () => void;
  readonly focusSearch: () => void;
}

export function useShortcuts(props: ShortcutProps): void {
  const { studio, state, view, keys, consoleOpen, setConsole, focusConsole, focusSearch } = props;
  const busy = state.busy.length > 0;
  useEffect(() => {
    const onKey = (event: Event): void => {
      if (!(event instanceof KeyboardEvent)) return;
      const what = shortcutOf(event.key, typingAt(event.composedPath()));
      if (what === null) return;
      // The browser's own meaning is never wanted here: `/` opens quick find, `f` types.
      event.preventDefault();
      if (what === "console") {
        setConsole(!consoleOpen);
        focusConsole();
      } else if (what === "search") {
        focusSearch();
      } else if (what === "fit") {
        void studio.dispatch("view.fit");
      } else if (consoleOpen) {
        setConsole(false);
      } else if (busy) {
        void studio.dispatch("view.cancel");
      } else {
        view.select(-1);
      }
    };
    keys.addEventListener("keydown", onKey);
    return () => keys.removeEventListener("keydown", onKey);
  }, [studio, view, keys, consoleOpen, busy, setConsole, focusConsole, focusSearch]);
}
