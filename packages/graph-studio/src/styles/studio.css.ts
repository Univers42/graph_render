/**
 * The whole stylesheet, as one string: the host puts it in a `<style>` inside the shadow
 * root, so nothing here can reach the page and nothing on the page reaches in.
 *
 * No backdrop-filter, no filter, no animation: each of them makes the browser recomposite
 * the canvas under the panels on every frame the graph draws.
 */
export const STUDIO_CSS = `
.gs-chrome {
  --gs-panel: #1e1e22;
  --gs-panel-2: #26262b;
  --gs-border: rgba(255,255,255,.08);
  --gs-text: #dadada;
  --gs-muted: #8e8e96;
  --gs-accent: #8b7cf6;
  --gs-danger: #e5484d;
  --gs-radius: 8px;
  position: absolute;
  inset: 0;
  pointer-events: none;
  opacity: 1;
  color: var(--gs-text);
  font: 12px/1.45 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
}
.gs-chrome[data-theme="light"] {
  --gs-panel: #f6f6f4;
  --gs-panel-2: #ecece9;
  --gs-border: rgba(0,0,0,.10);
  --gs-text: #2a2a2e;
  --gs-muted: #6c6c74;
  --gs-accent: #6c5ce0;
  --gs-danger: #c62d33;
}
.gs-chrome *, .gs-chrome *::before, .gs-chrome *::after { box-sizing: border-box; }
/* A class that sets display wins over the hidden attribute: a closed section stayed open. */
.gs-chrome [hidden] { display: none !important; }

.gs-panel {
  position: absolute;
  pointer-events: auto;
  opacity: 1;
  background: var(--gs-panel);
  border: 1px solid var(--gs-border);
  border-radius: var(--gs-radius);
  box-shadow: 0 8px 24px rgba(0,0,0,.35);
}
.gs-left { position: absolute; top: 12px; left: 12px; width: 260px; display: flex; flex-direction: column; gap: 8px; }
.gs-left .gs-panel { position: static; }
.gs-bottom-left { position: absolute; left: 12px; bottom: 12px; display: flex; flex-direction: column; align-items: flex-start; gap: 8px; }
.gs-bottom-left .gs-panel { position: static; }
.gs-dock { top: 12px; right: 12px; width: 280px; max-height: calc(100% - 24px); display: flex; flex-direction: column; z-index: 1; }
.gs-toast { top: 12px; left: 50%; transform: translateX(-50%); width: min(560px, calc(100% - 24px)); z-index: 3; }
.gs-console { right: 12px; bottom: 12px; width: min(720px, calc(100% - 24px)); height: 240px; display: flex; flex-direction: column; z-index: 2; }

.gs-head { display: flex; align-items: center; gap: 6px; padding: 6px 8px; border-bottom: 1px solid var(--gs-border); color: var(--gs-muted); }
.gs-head-name { flex: 1 1 auto; font-weight: 600; }
.gs-console, .gs-hud { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; }
.gs-hud { padding: 4px 8px; color: var(--gs-muted); white-space: nowrap; }
.gs-muted { color: var(--gs-muted); }

.gs-btn {
  height: 26px;
  padding: 0 8px;
  font: inherit;
  color: var(--gs-text);
  background: var(--gs-panel-2);
  border: 1px solid var(--gs-border);
  border-radius: 6px;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  transition: border-color 120ms ease, background-color 120ms ease;
}
.gs-btn:hover:not([disabled]) { border-color: var(--gs-accent); }
.gs-btn[aria-pressed="true"] { background: var(--gs-accent); border-color: var(--gs-accent); color: #ffffff; }
.gs-btn[disabled], .gs-input[disabled], .gs-select[disabled], .gs-range[disabled], .gs-check[disabled] { opacity: .45; cursor: not-allowed; }
.gs-btn:focus-visible, .gs-input:focus-visible, .gs-select:focus-visible, .gs-range:focus-visible, .gs-check:focus-visible { outline: 2px solid var(--gs-accent); outline-offset: 1px; }
.gs-input, .gs-select {
  height: 26px;
  width: 100%;
  padding: 0 6px;
  font: inherit;
  color: var(--gs-text);
  background: var(--gs-panel-2);
  border: 1px solid var(--gs-border);
  border-radius: 6px;
}
.gs-range { width: 100%; height: 26px; accent-color: var(--gs-accent); }
.gs-check { width: 16px; height: 16px; accent-color: var(--gs-accent); }

.gs-field { display: grid; grid-template-columns: 84px 1fr; align-items: center; gap: 6px; padding: 2px 8px; }
.gs-field-label { color: var(--gs-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gs-form { display: flex; flex-direction: column; gap: 4px; padding: 4px 0 6px; }
.gs-section-body, .gs-results { display: flex; flex-direction: column; }
.gs-list { display: grid; grid-template-columns: 1fr 1fr; gap: 4px; }
.gs-row { display: flex; gap: 4px; }
.gs-row > * { flex: 1 1 0; min-width: 0; }
.gs-form > .gs-row, .gs-form > .gs-list { padding: 0 8px; }
.gs-form > .gs-btn { margin: 0 8px; }
.gs-reason { padding: 0 8px 6px; color: var(--gs-danger); }
.gs-value { color: var(--gs-muted); font-variant-numeric: tabular-nums; }
.gs-dock-body { overflow-y: auto; }
.gs-section-head { display: flex; align-items: center; width: 100%; justify-content: space-between; background: transparent; border: 0; border-radius: 0; color: var(--gs-muted); text-align: left; }
.gs-section-body { padding-bottom: 6px; }
.gs-actions { display: flex; flex-direction: column; gap: 2px; }
.gs-action-title { padding: 2px 8px; color: var(--gs-muted); font-weight: 600; }
.gs-action { display: flex; flex-direction: column; }
.gs-action > .gs-btn { width: 100%; border-radius: 0; }

.gs-results { max-height: 180px; overflow-y: auto; border-top: 1px solid var(--gs-border); }
.gs-result { width: 100%; border: 0; border-radius: 0; text-align: left; }
.gs-search { display: flex; flex-direction: column; }
.gs-kv { display: grid; grid-template-columns: 60px 1fr; gap: 6px; padding: 1px 8px; }
.gs-kv-key { color: var(--gs-muted); }
.gs-kv-value, .gs-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gs-title { margin: 0; padding: 6px 8px; font-size: 12px; font-weight: 600; }
.gs-neighbours { display: flex; flex-wrap: wrap; gap: 4px; padding: 4px 8px 6px; }
.gs-legend { min-width: 180px; }
.gs-legend-row { display: grid; grid-template-columns: 10px 1fr auto; align-items: center; gap: 6px; padding: 1px 8px; }
.gs-swatch { width: 10px; height: 10px; border-radius: 2px; }
.gs-count { color: var(--gs-muted); font-variant-numeric: tabular-nums; }
.gs-hud-frame { font-variant-numeric: tabular-nums; }

.gs-log { flex: 1 1 auto; overflow-y: auto; padding: 4px 8px; }
.gs-entry { padding: 2px 0; }
.gs-line { display: grid; grid-template-columns: 1fr auto; gap: 8px; }
.gs-failed .gs-cmd { color: var(--gs-danger); }
.gs-failed .gs-line { color: var(--gs-danger); }
.gs-note, .gs-hint, .gs-detail { color: var(--gs-muted); }
.gs-candidates { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; padding: 4px 8px; border-top: 1px solid var(--gs-border); }
.gs-candidate { padding: 2px 6px; background: var(--gs-panel-2); border: 1px solid var(--gs-border); border-radius: 4px; }
.gs-more { color: var(--gs-muted); align-self: center; }
.gs-console-form { display: flex; gap: 6px; padding: 6px 8px; border-top: 1px solid var(--gs-border); }
.gs-busy { display: flex; align-items: center; gap: 8px; padding: 6px 8px; }
.gs-busy-name { flex: 1 1 auto; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.gs-alert { display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; }
.gs-alert-head { display: flex; align-items: center; gap: 6px; }
.gs-alert-title { flex: 1 1 auto; color: var(--gs-danger); font-weight: 600; }

@media (prefers-reduced-motion: reduce) {
  .gs-btn { transition: none; }
}
`;
