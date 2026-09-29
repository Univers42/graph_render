/**
 * Every key the studio answers, as rows: the camera keys from the navigation table, then
 * the keys the chrome owns. The overlay draws this list and nothing else, so a key added to
 * either table shows up without a second edit.
 */
import { KEYS } from "./navKeys.ts";

export interface Binding {
  readonly key: string;
  readonly id: string;
  readonly title: string;
}

/** The chrome's keys (useShortcuts.ts `chromeOf`); Escape is in KEYS as `view.clear`. */
export const CHROME_KEYS: readonly Binding[] = [
  { key: "`", id: "console.toggle", title: "Toggle the console" },
  { key: "/", id: "search.focus", title: "Focus the search" },
  { key: "?", id: "help.keymap", title: "Show the key bindings" },
];

export function keyBindings(titles: ReadonlyMap<string, string>): readonly Binding[] {
  const camera = Object.entries(KEYS).map(([key, nav]) => ({ key, id: nav.id, title: titles.get(nav.id) ?? nav.id }));
  return [...camera, ...CHROME_KEYS];
}
