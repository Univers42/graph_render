/** The colours the painter uses. The host owns them; the renderer reads no CSS. */

export interface Theme {
  readonly background: string;
  readonly edge: string;
  /** Edges of the hovered or selected node. */
  readonly edgeLit: string;
  /** The ring around the hovered or selected node. */
  readonly ring: string;
  /** Outline between touching boxes. */
  readonly rim: string;
  readonly label: string;
  readonly labelHalo: string;
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
  labelFont: '12px ui-sans-serif, system-ui, "Segoe UI", sans-serif',
  dimAlpha: 0.16,
};

export const LIGHT_THEME: Theme = {
  background: "#fbfbfc",
  edge: "rgba(90, 92, 105, 0.30)",
  edgeLit: "rgba(109, 40, 217, 0.95)",
  ring: "#6d28d9",
  rim: "#fbfbfc",
  label: "#26272b",
  labelHalo: "#fbfbfc",
  labelFont: '12px ui-sans-serif, system-ui, "Segoe UI", sans-serif',
  dimAlpha: 0.16,
};
