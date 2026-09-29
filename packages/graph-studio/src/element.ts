/**
 * <graph-studio>: the whole studio as one custom element, so a host is a tag and two
 * attributes. Everything it draws is inside its own shadow root: the host page's styles
 * do not reach the panels, and the panels' styles do not reach the host page.
 *
 *   <graph-studio wasm="/graph_wasm.wasm" fixtures="/fixtures/" keys="page"></graph-studio>
 *
 * `keys="page"` listens for shortcuts on the window; without it only while the element or
 * something in it has the focus, which is what a page that embeds it next to its own
 * inputs wants.
 */
import { createElement } from "react";
import { type Root, createRoot } from "react-dom/client";

import { type View, createView } from "../../graph-render/src/view.ts";
import type { Save } from "./actions/context.ts";
import { createClient } from "./motor/client.ts";
import type { Assets, Spawn } from "./motor/protocol.ts";
import { workerPort } from "./motor/workerPort.ts";
import { type Studio, createStudio } from "./studio/studio.ts";
import { STUDIO_CSS } from "./styles/studio.css.ts";
import { Shell } from "./ui/Shell.tsx";

const themeUnsub = new WeakMap<HTMLElement, () => void>();

export interface StudioElementOptions {
  /** Where the motor runs; a worker when left out. */
  readonly spawn?: Spawn;
  /** What an export does with its file; a download when left out. */
  readonly save?: Save;
}

export interface GraphStudioElement extends HTMLElement {
  /** `null` while the element is not in a document. */
  readonly studio: Studio | null;
  /**
   * The view the studio draws on, so a host can move the camera or read it. `null` while
   * the element is not in a document. The studio keeps its own; this is the same one.
   */
  readonly view: View | null;
}

interface Mounted {
  readonly studio: Studio;
  readonly view: View;
  readonly root: Root;
}

const HOST_CSS = `
:host { display: block; position: fixed; inset: 0; background: var(--gs-bg); color-scheme: var(--gs-color-scheme); outline: none; }
.gs-canvas { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
.gs-root { position: absolute; inset: 0; pointer-events: none; }
`;

const REVOKE_AFTER_MS = 60_000;

function spawnWorker(): ReturnType<Spawn> {
  return workerPort(new Worker(new URL("./motor/worker.ts", import.meta.url), { type: "module" }));
}

// Ponytail: the object URL is released a minute after the click, because a click only
// starts a download and nothing says when it ended. A download that takes longer than
// that to START is cut short; the largest export here is a PNG of the canvas.
function download(name: string, data: Blob): void {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(data);
  link.download = name;
  link.click();
  setTimeout(() => URL.revokeObjectURL(link.href), REVOKE_AFTER_MS);
}

function within(tag: string, className: string): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  return element;
}

/** Made absolute here: the worker would resolve them against its own script, not the page. */
function assetsOf(host: HTMLElement): Assets {
  const absolute = (name: string, fallback: string): string => new URL(host.getAttribute(name) ?? fallback, document.baseURI).href;
  return { wasmUrl: absolute("wasm", "graph_wasm.wasm"), fixturesUrl: absolute("fixtures", "fixtures/") };
}

function mount(host: HTMLElement, options: StudioElementOptions): Mounted {
  const shadow = host.shadowRoot ?? host.attachShadow({ mode: "open" });
  const style = document.createElement("style");
  style.textContent = `${HOST_CSS}${STUDIO_CSS}`;
  const canvas = document.createElement("canvas");
  canvas.className = "gs-canvas";
  const chrome = within("div", "gs-root");
  shadow.replaceChildren(style, canvas, chrome);
  // Focusable, so a click on the graph brings the shortcuts to this studio and no other.
  if (!host.hasAttribute("tabindex")) host.tabIndex = 0;
  const view = createView(canvas);
  const studio = createStudio({
    client: createClient(options.spawn ?? spawnWorker, assetsOf(host)),
    view,
    save: options.save ?? download,
    now: () => performance.now(),
  });
  const root = createRoot(chrome);
  root.render(createElement(Shell, { studio, view, keys: host.getAttribute("keys") === "page" ? window : host }));
  void studio.start();

  // Keep host CSS variables in sync with the studio's theme so :host { background: var(--gs-bg); color-scheme: var(--gs-color-scheme) } works.
  // Ponytail: the theme background colors are hardcoded here (from graph-render/src/theme.ts) rather than imported,
  // so a theme change in theme.ts must be mirrored here. The unsubscribe is stored on the host as a private
  // property; if multiple <graph-studio> elements exist, unmount() uses document.querySelector which finds
  // the first one, potentially cleaning up the wrong subscription. A proper fix would store the unsubscribe
  // on the Mounted object and call it directly from unmount(mounted).
  const THEME_BG = { dark: "#1b1b1f", light: "#fbfbfc" } as const;
  const applyTheme = (theme: "dark" | "light"): void => {
    host.style.setProperty("--gs-bg", THEME_BG[theme]);
    host.style.setProperty("--gs-color-scheme", theme);
  };
  applyTheme(studio.store.get().settings.appearance.theme);
  const unsubscribe = studio.store.subscribe(() => applyTheme(studio.store.get().settings.appearance.theme));
  // Store unsubscribe on the host for cleanup.
  themeUnsub.set(host, unsubscribe);

  return { studio, view, root };
}

function unmount(mounted: Mounted | null): void {
  if (mounted === null) return;
  mounted.root.unmount();
  mounted.studio.destroy();
  mounted.view.destroy();
  // Clean up theme subscription on the host element.
  const host = document.querySelector<HTMLElement>("graph-studio");
  if (host) {
    const unsub = themeUnsub.get(host);
    if (unsub) unsub();
  }
}

/** Registers the element once; a second call, or a tag already taken, changes nothing. */
export function defineGraphStudio(options: StudioElementOptions = {}, tag = "graph-studio"): void {
  if (customElements.get(tag) !== undefined) return;
  customElements.define(tag, class extends HTMLElement implements GraphStudioElement {
    #mounted: Mounted | null = null;

    get studio(): Studio | null {
      return this.#mounted?.studio ?? null;
    }

    get view(): View | null {
      return this.#mounted?.view ?? null;
    }

    connectedCallback(): void {
      this.#mounted ??= mount(this, options);
    }

    disconnectedCallback(): void {
      unmount(this.#mounted);
      this.#mounted = null;
    }
  });
}
