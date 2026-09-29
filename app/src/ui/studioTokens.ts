/**
 * The studio's `--osio-*` design tokens.
 *
 * `resolveSceneTheme` reads these from the document root at runtime, which is how
 * the engine follows a host app's palette. The studio IS the host here, so it
 * declares the aurora-glass dark field once and the whole render layer — the
 * background, the node sprites, the labels, the rings — follows from it. A
 * palette change is a change to this table and nothing else.
 */

const TOKENS: Readonly<Record<string, string>> = {
  "--osio-graph-bg-0": "#1d1a14",
  "--osio-graph-bg-1": "#12110d",
  "--osio-graph-field-warm": "rgba(224, 147, 122, 0.12)",
  "--osio-graph-vignette": "rgba(0, 0, 0, 0.45)",
  "--osio-graph-grain": "rgba(255, 240, 210, 0.03)",
  "--osio-graph-edge": "rgba(198, 190, 176, 0.28)",
  "--osio-graph-edge-tag": "rgba(217, 184, 156, 0.2)",
  "--osio-graph-edge-note": "rgba(224, 147, 122, 0.6)",
  "--osio-graph-edge-hier": "rgba(233, 205, 176, 0.8)",
  "--osio-graph-select": "#e0937a",
  "--osio-graph-hover": "rgba(224, 147, 122, 0.6)",
  "--osio-graph-note": "#c9a227",
  "--osio-graph-node-backing": "rgba(24, 21, 16, 0.72)",
  "--osio-graph-node-rim": "rgba(255, 244, 228, 0.18)",
  "--osio-graph-node-shadow": "rgba(0, 0, 0, 0.5)",
  "--osio-fg-default": "#edeae3",
  "--osio-fg-muted": "#a9a296",
  "--osio-accent": "#e0937a",
  "--osio-surface": "#191712",
  "--osio-surface-raised": "#221f18",
  "--osio-border": "rgba(255, 240, 210, 0.12)",
};

/** Publish the tokens on the document root. Idempotent. */
export function studioTokens(root: HTMLElement = document.documentElement): void {
  for (const [name, value] of Object.entries(TOKENS)) root.style.setProperty(name, value);
}
