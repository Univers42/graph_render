/**
 * One studio inside one `<graph-studio>`, built and taken down as a unit: the shadow root, the
 * motor client, the view, the chrome and the host API's watchers (`element.ts` holds the result).
 */
import { createElement } from "react";
import { type Root, createRoot } from "react-dom/client";

import { type BackendChoice, type View, createView } from "../../graph-render/src/view.ts";
import { type HostVerbs, hostVerbs } from "./host/api.ts";
import type { Resolve } from "./host/contract.ts";
import { emit } from "./host/events.ts";
import { watchGestures } from "./host/gestures.ts";
import { type Previews, createPreviews } from "./host/previews.ts";
import { watchHost } from "./host/watch.ts";
import { type LiveBridge, NOT_ASKED, createLiveBridge, watchRuns } from "./motor/bridge.ts";
import { type MotorClient, createClient } from "./motor/client.ts";
import { createLiveDrag } from "./motor/liveDrag.ts";
import type { Assets, Spawn } from "./motor/protocol.ts";
import { workerPort } from "./motor/workerPort.ts";
import type { Save } from "./actions/context.ts";
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
  /** Threads that tick a live settle, the motor worker's own included (`motor/threads.ts`); one per core but one, at most 8, when left out. */
  readonly threads?: number;
}

export interface Mounted {
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
  /** What a host calls; `docs/contract/host-api.md`. */
  readonly verbs: HostVerbs;
  /** The previews behind `resolve`, for `invalidate`. */
  readonly previews: Previews;
  /** Stops the host's events and gestures, and aborts any `resolve` in flight. */
  readonly unhost: () => void;
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

/** Made absolute here: the worker would resolve them against its own script, not the page. */
function assetsOf(host: HTMLElement, threads: number | undefined): Assets {
  const absolute = (name: string, fallback: string): string => new URL(host.getAttribute(name) ?? fallback, document.baseURI).href;
  const assets = { wasmUrl: absolute("wasm", "graph_wasm.wasm"), fixturesUrl: absolute("fixtures", "fixtures/") };
  return threads === undefined ? assets : { ...assets, threads };
}

/**
 * `localStorage` when the host asked for `remember`, else null; null too where reading the
 * property itself throws (blocked site data).
 *
 * WHY opt-in (verdict 3): an embedded studio that reopened the last graph by itself would draw,
 * and announce `graph-load`, before its host had said what to show.
 */
function pageStorage(host: HTMLElement): SettingsStorage | null {
  if (!host.hasAttribute("remember")) return null;
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

/** What the parts made before the studio need from it, read late through this. */
interface Shown {
  studio: Studio | null;
  /** One line in the console log, naming why a live session ended. */
  note(reason: string): void;
}

/**
 * The live bridge and the view it paints, wired together and handed back: a frame from the
 * worker goes to `view.setPositions` and the forces link is the bridge's.
 */
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

function shadowOf(host: HTMLElement): { readonly canvas: HTMLCanvasElement; readonly chrome: HTMLElement } {
  const shadow = host.shadowRoot ?? host.attachShadow({ mode: "open" });
  // A constructed sheet, not a <style> element: the CSP a host is asked for (host-api.md, verdict
  // 13) has no 'unsafe-inline', and Chromium refused the <style> under it (studio-embed, csp run).
  const sheet = new CSSStyleSheet();
  sheet.replaceSync(`${HOST_CSS}${STUDIO_CSS}`);
  shadow.adoptedStyleSheets = [sheet];
  const canvas = document.createElement("canvas");
  canvas.className = "gs-canvas";
  const chrome = document.createElement("div");
  chrome.className = "gs-root";
  shadow.replaceChildren(canvas, chrome);
  // Focusable, so a click on the graph brings the shortcuts to this studio and no other.
  if (!host.hasAttribute("tabindex")) host.tabIndex = 0;
  return { canvas, chrome };
}

type Parts = Pick<Mounted, "studio" | "view" | "client" | "bridge">;

function studioOf(host: HTMLElement, options: StudioElementOptions, canvas: HTMLCanvasElement): Parts {
  const client = createClient(options.spawn ?? spawnWorker, assetsOf(host, options.threads));
  // The view is made before the studio, and the ids live in the studio's state: read late.
  const shown: Shown = { studio: null, note: (reason) => shown.studio?.note(reason) };
  const { view, bridge } = livePair(canvas, client, shown, options.backend ?? "auto");
  const storage = pageStorage(host);
  const studio = createStudio({
    client,
    view,
    save: options.save ?? download,
    now: () => performance.now(),
    forces: bridge.link,
    open: (id, via) => emit(host, "node-open", { id, via }),
    ...(storage === null ? {} : { storage, settings: openingSettings(storage) }),
  });
  shown.studio = studio;
  return { studio, view, client, bridge };
}

/** `resolve` is read through `resolver` on every call, so the host may set it at any time. */
export function mount(host: HTMLElement, options: StudioElementOptions, resolver: () => Resolve | null): Mounted {
  const { canvas, chrome } = shadowOf(host);
  const { studio, view, client, bridge } = studioOf(host, options, canvas);
  const previews = createPreviews({ resolver });
  // Before `start`: a studio that remembers draws at once, and that load is announced too.
  const unwatchHost = watchHost({ host, store: studio.store, view, previews });
  const unwatchGestures = watchGestures({ host, canvas, studio, view });
  const unwatch = watchRuns(studio.store, bridge);
  const root = createRoot(chrome);
  root.render(createElement(Shell, {
    studio, view, keys: host.getAttribute("keys") === "page" ? window : host, bar: bridge, previews,
  }));
  const started = studio.start(host.hasAttribute("remember"));
  // The arrow, not the method: `watchSafeArea` holds this until unmount, and a bare method
  // reference would leave `this` to chance — `view.setSafeArea(area)` names the receiver.
  const unwatchArea = watchSafeArea(canvas, chrome, (area) => view.setSafeArea(area));
  const unhost = (): void => {
    unwatchHost();
    unwatchGestures();
    previews.clear();
  };
  return { studio, view, client, root, bridge, unwatch, unwatchArea, verbs: hostVerbs(studio, started), previews, unhost };
}

export function unmount(mounted: Mounted | null): void {
  if (mounted === null) return;
  mounted.unhost();
  mounted.root.unmount();
  mounted.unwatchArea();
  mounted.unwatch();
  mounted.bridge.destroy();
  mounted.studio.destroy();
  mounted.view.destroy();
}
