/**
 * The themes the studio offers by name: its own dark and light, two of ours that sit near
 * the note-taking look, and every SciGraphs preset through the one encode in theme.ts.
 * The obsidian values are ours, chosen for contrast; no code or asset comes from the
 * application they are named after.
 */
import { DARK_THEME, LIGHT_THEME, type Theme } from "../theme.ts";
import { LABEL_HEIGHT } from "../labels.ts";
import { LOOK_NAMES, lookOf } from "./presets.ts";
import { lookTheme } from "./theme.ts";

const FONT = '12px ui-sans-serif, system-ui, "Segoe UI", sans-serif';

export const OBSIDIAN_DARK: Theme = {
  background: "#1e1e1e",
  edge: "rgba(160, 160, 160, 0.30)",
  edgeLit: "rgba(139, 108, 239, 0.95)",
  ring: "#8b6cef",
  rim: "#1e1e1e",
  label: "#dadada",
  labelHalo: "#1e1e1e",
  labelBox: null,
  labelHeight: LABEL_HEIGHT,
  labelFont: FONT,
  dimAlpha: 0.16,
};

export const OBSIDIAN_LIGHT: Theme = {
  background: "#ffffff",
  edge: "rgba(100, 100, 100, 0.30)",
  edgeLit: "rgba(112, 82, 210, 0.95)",
  ring: "#7052d2",
  rim: "#ffffff",
  label: "#222222",
  labelHalo: "#ffffff",
  labelBox: null,
  labelHeight: LABEL_HEIGHT,
  labelFont: FONT,
  dimAlpha: 0.16,
};

export const THEME_NAMES: readonly string[] = ["dark", "light", "obsidian-dark", "obsidian-light", ...LOOK_NAMES];

const OWN: Readonly<Record<string, Theme>> = {
  dark: DARK_THEME, light: LIGHT_THEME, "obsidian-dark": OBSIDIAN_DARK, "obsidian-light": OBSIDIAN_LIGHT,
};

export function themeNamed(name: string): Theme {
  const own = Object.hasOwn(OWN, name) ? OWN[name] : undefined;
  if (own !== undefined) return own;
  return LOOK_NAMES.includes(name) ? lookTheme(lookOf(name)) : DARK_THEME;
}

/**
 * Ponytail: light is a name list, not a luminance test; a preset added to presets.ts is
 * dark to the chrome until it is listed here. Only `paper` is light among the presets.
 */
const LIGHT = new Set(["light", "obsidian-light", "paper"]);

export function isLightTheme(name: string): boolean {
  return LIGHT.has(name);
}
