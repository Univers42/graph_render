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
import { type DeltasPage, createDeltasPage } from "./motor/deltasPage.ts";
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
  /** The page's half of a delta batch: the structure it needs drawn, and the frame up to it. */
  readonly deltas: DeltasPage;
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

// Caveat: a download whose START takes longer than a minute is cut short, because the object
// URL is released a minute after the click and a click only starts a download, nothing saying
// when it ended. It then fails to begin rather than arriving short; create and revoke the
// object URL yourself once the write is known to have begun. The largest export here is a PNG
// of the canvas.
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
  // `?break-deltas=1` is the gate's negative control and nothing else: the worker drops the grow
  // after an extend, so the batch is applied and the new nodes never move.
  const gate = new URL(document.baseURI).searchParams.get("break-deltas") === "1" ? { breakDeltas: true } : {};
  return threads === undefined ? { ...assets, ...gate } : { ...assets, threads, ...gate };
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
  /** One line in the console log that is not a failure. */
  tell(line: string): void;
}

/**
 * The live bridge and the view it paints, wired together and handed back: a frame from the
 * worker goes to `view.setPositions` and the forces link is the bridge's.
 */
function livePair(canvas: HTMLCanvasElement, client: MotorClient, shown: Shown, backend: BackendChoice): {
  readonly view: View;
  readonly bridge: LiveBridge;
  readonly page: DeltasPage;
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
  // The store is the studio's and the studio is made after this, so both are read late.
  const page = createDeltasPage(
    view,
    () => shown.studio?.store.get().meta ?? null,
    (meta) => { shown.studio?.store.update((state) => ({ ...state, meta })); },
  );
  const bridge = createLiveBridge({
    send: (request) => client.force?.(request),
    onPush: (handler) => client.onForce?.(handler) ?? (() => undefined),
    onFail: (handler) => client.onFail?.(handler) ?? (() => undefined),
    paint: (frame) => page.frame(frame.xs, frame.ys),
    // The structure a delta batch needs drawn; the frames above are drawn up to its count.
    structure: (run) => page.structure(run),
    report: (reason) => shown.note(reason),
    inform: (line) => shown.tell(line),
  });
  wires.bridge = bridge;
  return { view, bridge, page };
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

type Parts = Pick<Mounted, "studio" | "view" | "client" | "bridge" | "deltas">;

/**
 * The parts a mount has made so far, every member optional: a `throw` part-way leaves only what was
 * made before it, and `release` is what takes those back.
 *
 * Each member is narrowed to the one release step `release` calls on it, which is also all
 * `unmount` ever calls: the parts that take no argument and return nothing.
 *
 * No member for the client: its worker is spawned on the motor's first call, which happens inside
 * `studio.start` — after the studio is on here, and `studio.destroy` closes it.
 */
export interface Building {
  studio?: Pick<Studio, "destroy">;
  view?: Pick<View, "destroy">;
  bridge?: Pick<LiveBridge, "destroy">;
  root?: Pick<Root, "unmount">;
  unwatch?: () => void;
  unwatchArea?: () => void;
  unhost?: () => void;
}

/**
 * The one definition of released. `unmount` and a mount that threw part-way both call this, so the
 * two cannot drift: the steps are `unmount`'s, in `unmount`'s order, and each one runs only if its
 * part was made. A studio here ends its worker; the rest stop watching and free what they made.
 */
export function release(parts: Building): void {
  parts.unhost?.();
  parts.root?.unmount();
  parts.unwatchArea?.();
  parts.unwatch?.();
  parts.bridge?.destroy();
  parts.studio?.destroy();
  parts.view?.destroy();
}

function studioOf(host: HTMLElement, options: StudioElementOptions, canvas: HTMLCanvasElement): Parts {
  const client = createClient(options.spawn ?? spawnWorker, assetsOf(host, options.threads));
  // The view is made before the studio, and the ids live in the studio's state: read late.
  const shown: Shown = {
    studio: null,
    note: (reason) => shown.studio?.note(reason),
    tell: (line) => shown.studio?.tell(line),
  };
  const { view, bridge, page } = livePair(canvas, client, shown, options.backend ?? "auto");
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
  return { studio, view, client, bridge, deltas: page };
}

/** `resolve` is read through `resolver` on every call, so the host may set it at any time. */
export function mount(host: HTMLElement, options: StudioElementOptions, resolver: () => Resolve | null): Mounted {
  const parts: Building = {};
  try {
    const { canvas, chrome } = shadowOf(host);
    const { studio, view, client, bridge, deltas } = studioOf(host, options, canvas);
    Object.assign(parts, { studio, view, bridge });
    const previews = createPreviews({ resolver });
    // Grown as the parts are made, so a throw between two of them releases both. The release steps
    // themselves are `release`'s and `unmount`'s, and none of them is written out twice here.
    const unhosted: (() => void)[] = [() => previews.clear()];
    // Before `start`: a studio that remembers draws at once, and that load is announced too.
    const unwatchHost = watchHost({ host, store: studio.store, view, previews });
    unhosted.push(unwatchHost);
    const unwatchGestures = watchGestures({ host, canvas, studio, view });
    unhosted.push(unwatchGestures);
    const unhost = (): void => {
      for (const stop of unhosted) stop();
    };
    parts.unhost = unhost;
    const unwatch = watchRuns(studio.store, bridge);
    parts.unwatch = unwatch;
    const root = createRoot(chrome);
    parts.root = root;
    root.render(createElement(Shell, {
      studio, view, keys: host.getAttribute("keys") === "page" ? window : host, bar: bridge, previews,
    }));
    const started = studio.start(host.hasAttribute("remember"));
    // The arrow, not the method: `watchSafeArea` holds this until unmount, and a bare method
    // reference would leave `this` to chance — `view.setSafeArea(area)` names the receiver.
    const unwatchArea = watchSafeArea(canvas, chrome, (area) => view.setSafeArea(area));
    parts.unwatchArea = unwatchArea;
    return { studio, view, client, root, bridge, deltas, unwatch, unwatchArea, verbs: hostVerbs(host, studio, started), previews, unhost };
  } catch (error) {
    // `element.ts` hands `#mounted` only what this returns, so a throw part-way leaves the host
    // with nothing to unmount: what was made is released here, and the error raised unchanged.
    release(parts);
    throw error;
  }
}

export function unmount(mounted: Mounted | null): void {
  if (mounted === null) return;
  release(mounted);
}
