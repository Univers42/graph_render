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

import { createLiveDrag } from "./motor/liveDrag.ts";
import { type BackendChoice, type View, createView } from "../../graph-render/src/view.ts";

/** The host reads `?backend=` with this, so it never imports the renderer itself. */
export { backendOf } from "../../graph-render/src/view.ts";
import type { Save } from "./actions/context.ts";
import { type LiveBridge, createLiveBridge, settlesLive } from "./motor/bridge.ts";
import { NOT_ASKED } from "./motor/bridge.ts";
import { type MotorClient, createClient } from "./motor/client.ts";
import { SILENCE_MS } from "./motor/watchdog.ts";
import type { Assets, Spawn } from "./motor/protocol.ts";
import { workerPort } from "./motor/workerPort.ts";
import { type SettingsStorage, openingSettings } from "./state/persist.ts";
import { type Studio, createStudio } from "./studio/studio.ts";
import { STUDIO_CSS } from "./styles/studio.css.ts";
import { Shell } from "./ui/Shell.tsx";
import { watchSafeArea } from "./ui/safeArea.ts";

export interface StudioElementOptions {
  /** Where the motor runs; a worker when left out. */
  readonly spawn?: Spawn;
  /** What an export does with its file; a download when left out. */
  readonly save?: Save;
  /** Who draws the graph's edges and nodes (graph-render `ViewOptions.backend`); `auto` when left out. */
  readonly backend?: BackendChoice;
}

export interface GraphStudioElement extends HTMLElement {
  /** `null` while the element is not in a document. */
  readonly studio: Studio | null;
  /**
   * The view the studio draws on, so a host can move the camera or read it. `null` while
   * the element is not in a document. The studio keeps its own; this is the same one.
   */
  readonly view: View | null;
  /**
   * Stops the motor worker where it stands and lets nothing replace it: the live settle
   * ends at once, and the watchdog puts the strip away and names the cause. This is the
   * one verb a gate needs to watch a dead worker from the outside; the studio never calls
   * it itself, and the next layout opens a new worker as usual.
   */
  stopMotor(): void;
  /**
   * How long the watchdog waits, in milliseconds, before it calls a silent worker dead.
   * Exposed so a gate can say "within the bound" without carrying its own copy of the
   * number, which would drift from it silently. Read-only, and not a setting.
   */
  readonly watchdogBoundMs: number;
}

interface Mounted {
  readonly studio: Studio;
  readonly view: View;
  readonly client: MotorClient;
  readonly root: Root;
  /** The live bridge: the drag, the forces panel and the progress strip all read it. */
  readonly bridge: LiveBridge;
  /** Stops watching the studio's state for a layout that settles live. */
  readonly unwatch: () => void;
  /** Stops measuring the panels over the canvas (ST-4). */
  readonly unwatchArea: () => void;
}

const HOST_CSS = `
:host { display: block; position: relative; overflow: hidden; outline: none; }
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

/** `localStorage`, or null where reading the property itself throws (blocked site data). */
function pageStorage(): SettingsStorage | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/**
 * The live bridge and the view it paints, wired together and handed back: a frame from the
 * worker goes to `view.setPositions` and the forces link is the bridge's.
 *
 * `shown.note` is filled in once the studio exists — the bridge is made first — so a watchdog
 * that fires later still has a console to write its one line into.
 */
/** What the parts made before the studio need from it, read late through this. */
interface Shown {
  studio: Studio | null;
  /** One line in the console log, naming why a live session ended. */
  note(reason: string): void;
}

function livePair(canvas: HTMLCanvasElement, client: MotorClient, shown: Shown, backend: BackendChoice): {
  readonly view: View;
  readonly bridge: LiveBridge;
} {
  const wires: { bridge: LiveBridge | null } = { bridge: null };
  // WHY an explicit `undefined` test and not `??`: the link answers `null` when the simulation
  // CAN run, and `null ?? x` is `x`, so that fallback reads a working session as a missing one
  // and every drag quietly falls back to the view-only one.
  const why = (): string | null => {
    const reason = wires.bridge?.link.disabled();
    return reason === undefined ? NOT_ASKED : reason;
  };
  const view = createView(canvas, {
    backend,
    live: createLiveDrag({
      ids: () => shown.studio?.store.get().meta?.ids ?? null,
      disabled: why,
      send: (request) => client.force?.(request),
    }),
  });
  const bridge = createLiveBridge({
    send: (request) => client.force?.(request),
    onPush: (handler) => client.onForce?.(handler) ?? (() => undefined),
    onFail: (handler) => client.onFail?.(handler) ?? (() => undefined),
    paint: (frame) => view.setPositions(frame.xs, frame.ys),
    report: (reason) => shown.note(reason),
  });
  wires.bridge = bridge;
  return { view, bridge };
}

/**
 * A force layout is a starting position, not a picture: the loop takes it from there and the
 * strip shows the settle. Every other layout is finished, so nothing starts. A batch layout
 * run shows the same strip with no fraction of its own — one call, no progress inside it.
 */
function watchRuns(studio: Studio, bridge: LiveBridge): () => void {
  let settled = "";
  let wasBusy = 0;
  return studio.store.subscribe(() => {
    const at = studio.store.get();
    if (at.busy.length !== wasBusy) {
      wasBusy = at.busy.length;
      bridge.batch(wasBusy);
    }
    const layoutId = at.run?.layoutId ?? "";
    if (layoutId === settled) return;
    settled = layoutId;
    if (settlesLive(layoutId)) bridge.start();
  });
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
  const client = createClient(options.spawn ?? spawnWorker, assetsOf(host));
  // The view is made before the studio, and the ids live in the studio's state: read late.
  const shown: Shown = { studio: null, note: (reason) => shown.studio?.note(reason) };
  const { view, bridge } = livePair(canvas, client, shown, options.backend ?? "auto");
  const storage = pageStorage();
  const studio = createStudio({
    client,
    view,
    save: options.save ?? download,
    now: () => performance.now(),
    forces: bridge.link,
    ...(storage === null ? {} : { storage, settings: openingSettings(storage) }),
  });
  shown.studio = studio;
  const unwatch = watchRuns(studio, bridge);
  const root = createRoot(chrome);
  root.render(createElement(Shell, {
    studio, view, keys: host.getAttribute("keys") === "page" ? window : host, bar: bridge,
  }));
  void studio.start();
  return { studio, view, client, root, bridge, unwatch, unwatchArea: watchSafeArea(canvas, chrome, view.setSafeArea) };
}

function unmount(mounted: Mounted | null): void {
  if (mounted === null) return;
  mounted.root.unmount();
  mounted.unwatchArea();
  mounted.unwatch();
  mounted.bridge.destroy();
  mounted.studio.destroy();
  mounted.view.destroy();
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

    stopMotor(): void {
      // `close`, not `destroy`: the studio and its chrome stay, so the page reads as a studio
      // that lost its motor rather than one that was taken down.
      this.#mounted?.client.close();
    }

    get watchdogBoundMs(): number {
      return SILENCE_MS;
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
