/** The colours the painter uses. The host owns them; the renderer reads no CSS. */
import { LABEL_HEIGHT } from "./labels.ts";

export interface LabelBox {
  /** The fill behind the text: the SciGraphs overlay's 0.6 black (text_overlay.py:547-562). */
  readonly fill: string;
  /** CSS pixels of clear space between the text and the box, on every side. */
  readonly padding: number;
}

/** What is painted behind the graph when the host wants something other than `Theme.background`. */
export type Backdrop =
  | { readonly mode: "flat"; readonly colour: string }
  | { readonly mode: "aurora"; readonly stops: readonly [string, string, string] };

export interface Theme {
  readonly background: string;
  /** Replaces `background` as the ground when present; the rim and label halo keep the theme's own. */
  readonly backdrop?: Backdrop;
  readonly edge: string;
  /** Edges of the hovered or selected node. */
  readonly edgeLit: string;
  /** The ring around the hovered or selected node. */
  readonly ring: string;
  /** Outline between touching boxes. */
  readonly rim: string;
  readonly label: string;
  readonly labelHalo: string;
  /** Filled behind the label text, or null for a halo stroke on the backdrop. */
  readonly labelBox: LabelBox | null;
  /** Sprite height of a baked label, in CSS pixels; padding included. */
  readonly labelHeight: number;
  readonly labelFont: string;
  /** Opacity of everything outside a lit neighbourhood. */
  readonly dimAlpha: number;
}

export const DARK_THEME: Theme = {
  background: "#1b1b1f",
  edge: "rgba(150, 152, 165, 0.34)",
  edgeLit: "rgba(167, 139, 250, 0.95)",
  ring: "#a78bfa",
  rim: "#1b1b1f",
  label: "#dcdde1",
  labelHalo: "#1b1b1f",
  labelBox: null,
  labelHeight: LABEL_HEIGHT,
  labelFont: '12px ui-sans-serif, system-ui, "Segoe UI", sans-serif',
  dimAlpha: 0.12,
};

export const LIGHT_THEME: Theme = {
  background: "#fbfbfc",
  edge: "rgba(90, 92, 105, 0.30)",
  edgeLit: "rgba(109, 40, 217, 0.95)",
  ring: "#6d28d9",
  rim: "#fbfbfc",
  label: "#26272b",
  labelHalo: "#fbfbfc",
  labelBox: null,
  labelHeight: LABEL_HEIGHT,
  labelFont: '12px ui-sans-serif, system-ui, "Segoe UI", sans-serif',
  dimAlpha: 0.12,
};
