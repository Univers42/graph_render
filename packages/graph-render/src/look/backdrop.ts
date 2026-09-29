/** The display panel's background modes as a theme: `theme` is the theme itself. */
import type { Backdrop, Theme } from "../theme.ts";

export type BackgroundMode = "theme" | "flat" | "aurora";

const FLAT_DARK = "#20232a";
const FLAT_LIGHT = "#e6e8ee";
const AURORA_DARK = ["#0f2027", "#203a43", "#2c5364"] as const;
const AURORA_LIGHT = ["#dbeafe", "#e0e7ff", "#fae8ff"] as const;

function backdropOf(mode: "flat" | "aurora", light: boolean): Backdrop {
  if (mode === "flat") return { mode, colour: light ? FLAT_LIGHT : FLAT_DARK };
  const [from, mid, to] = light ? AURORA_LIGHT : AURORA_DARK;
  return { mode, stops: [from, mid, to] };
}

/** `theme` returns `theme` itself, so an unchanged mode costs the view no repaint of its own. */
export function backdropTheme(theme: Theme, mode: BackgroundMode, light: boolean): Theme {
  return mode === "theme" ? theme : { ...theme, backdrop: backdropOf(mode, light) };
}
