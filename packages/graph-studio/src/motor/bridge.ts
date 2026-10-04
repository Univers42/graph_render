/**
 * The page's side of the live loop, in one object: the `ForceLink` the forces actions and
 * the drag reach, and the store the progress bar reads.
 *
 * The frames arrive here, not in the studio's state: a settle pushes one per animation
 * frame, and putting that in the store every panel reads would re-render the whole chrome
 * sixty times a second. So the bar has its own store with its own subscribers, and the
 * positions go straight to the view, which redraws on its own request.
 *
 * A watchdog sits on top of it: while the strip is up, every message from the worker re-arms a
 * bounded timer, and a timer that runs out means the worker is gone — so the strip is hidden,
 * the session is marked dead, and one console line says why.
 *
 * Ponytail: `disabled()` reads the last state the loop reported, so the panel is greyed
 * until the worker has answered one force request and knows whether it has a session.
 * Failing input: a worker that dies between frames leaves the last state standing, and the
 * panel goes on believing a loop is running; the watchdog is what rediscovers it, and
 * `forces.animate` is the manual way back.
 * Direction: the state is the worker's, not a guess made here. Escape hatch: `start` is
 * called after every force layout, so a re-layout makes a new session and a new loop.
 */
import type { ForceLink } from "../actions/forces.ts";
import { DEFAULT_KNOBS, type ForceKnobs, settlesLive } from "./live.ts";
import type { ForceFrame, ForceRequest, Result, RunReport } from "./protocol.ts";
import { type Watchdog, createWatchdog, later } from "./watchdog.ts";
import type { Store } from "../state/store.ts";
import { type Bar, HIDDEN, batchBar, frameBar } from "../ui/progress.ts";

export interface LiveDeps {
  /** Sends a force request; a drop is silent, so the state below is the worker's word. */
  readonly send: (request: ForceRequest) => void;
  /** Everything the motor pushes without being asked; returns the cancel. */
  readonly onPush: (handler: (result: Result) => void) => () => void;
  /** The columns to draw. */
  readonly paint: (frame: ForceFrame) => void;
  /** Why the loop cannot run, or null; asked per event, so a late session takes effect. */
  readonly unavailable?: () => string | null;
  /** The worker failing where the page can hear it; the watchdog's other way in. */
  readonly onFail?: (handler: (detail: string) => void) => () => void;
  /** One line in the console, naming why a live session ended; absent in a bare test. */
  readonly report?: (reason: string) => void;
  /**
   * The structure snapshot the worker rebuilt after a delta batch, so the new nodes are drawn.
   * Absent where nothing draws: the frames still paint.
   */
  readonly structure?: (run: RunReport) => void;
  /** Over the watchdog's timer; the wall clock when left out. */
  readonly schedule?: (run: () => void, ms: number) => () => void;
}

export interface LiveBridge {
  readonly link: ForceLink;
  /** The bar as it is now, and every change to it. */
  readonly bar: () => Bar;
  readonly onBar: (handler: (bar: Bar) => void) => () => void;
  /** Called after a force layout, so the next drag and the next frame have a session. */
  start(): void;
  /** How many calls the worker is running; the bar shows them the same way it shows a settle. */
  batch(count: number): void;
  destroy(): void;
}

const listeners = () => {
  let bar: Bar = HIDDEN;
  const watching = new Set<(bar: Bar) => void>();
  return {
    get: () => bar,
    set: (next: Bar): void => {
      if (next.visible === bar.visible && next.fraction === bar.fraction && next.label === bar.label) return;
      bar = next;
      for (const handler of watching) handler(bar);
    },
    on: (handler: (bar: Bar) => void): (() => void) => {
      watching.add(handler);
      return () => void watching.delete(handler);
    },
  };
};

/** Everything the bridge remembers, and the one line that puts it on screen. */
interface Desk {
  knobs: ForceKnobs;
  running: boolean;
  paused: boolean;
  /** Why the loop cannot run, or `undefined` before the worker has answered once. */
  available: string | null | undefined;
  /** The bar the last live frame asked for; `HIDDEN` when there is no settle. */
  settling: Bar;
  /** How many calls the worker is running. */
  busy: number;
  /** Re-arms the watchdog while the settle strip is up, and sleeps it when the strip is not. */
  watchdog: Watchdog;
}

export const NOT_ASKED = "the motor has not been asked yet";

function newDesk(): Desk {
  return {
    knobs: DEFAULT_KNOBS, running: false, paused: false, available: undefined, settling: HIDDEN, busy: 0,
    watchdog: { touch: () => undefined, rest: () => undefined, lost: () => undefined, stop: () => undefined },
  };
}

/**
 * The watchdog's one handler: a live session ended, so the strip goes, the panel is told why,
 * and one line reaches the console. Not sticky — the next message from the worker re-arms the
 * watchdog and `link.disabled()` goes back to null on its own.
 */
function dead(desk: Desk, deps: LiveDeps, publish: () => void, reason: string): void {
  // The last state standing is the worker's word, from a worker that is no longer there.
  desk.running = false;
  desk.paused = false;
  desk.available = reason;
  desk.settling = HIDDEN;
  publish();
  desk.watchdog.rest();
  deps.report?.(reason);
}

/** One strip, two things that can want it: the settle wins while it runs. */
function barOf(desk: Desk): Bar {
  return desk.busy > 0 && !desk.running ? batchBar(desk.busy) : desk.settling;
}

/**
 * Publishes the bar and arms the watchdog on it.
 *
 * The batch strip is not watched: a batch layout run reports no progress of its own and can
 * legitimately take longer than the bound, so a worker busy in a layout is not a dead worker.
 * Only a settle strip is a claim that messages are coming, so only it arms the timer.
 */
function show(desk: Desk, publish: () => void): void {
  publish();
  if (desk.settling.visible) desk.watchdog.touch();
  else desk.watchdog.rest();
}

/**
 * Why the loop cannot run, or `null` when it can.
 *
 * `undefined` is "the worker has not answered a force request yet" and `null` is "it answered,
 * and there is a session": the two must not share a `??` fallback, because `null ?? x` is `x`
 * and a working session would read as a missing one — which greys the panel and turns every
 * drag into a view-only one.
 */
function reasonFor(desk: Desk, deps: LiveDeps): string | null {
  const fromHost = deps.unavailable?.();
  if (fromHost !== undefined) return fromHost;
  return desk.available === undefined ? NOT_ASKED : desk.available;
}

/** The forces link over one desk: what the panel's sliders and the console commands do. */
function linkOf(desk: Desk, deps: LiveDeps, publish: () => void): ForceLink {
  return {
    disabled: () => reasonFor(desk, deps),
    knobs: () => desk.knobs,
    set: (next, heat) => {
      desk.knobs = next;
      deps.send(heat === undefined ? { type: "force.params", knobs: next } : { type: "force.params", knobs: next, heat });
    },
    animate: (on) => {
      desk.running = on;
      deps.send({ type: "force.start" });
      if (on) return;
      deps.send({ type: "force.stop" });
      desk.settling = HIDDEN;
      show(desk, publish);
    },
    animating: () => desk.running && !desk.paused,
    pause: () => {
      desk.paused = true;
      deps.send({ type: "force.pause" });
    },
    resume: () => {
      desk.paused = false;
      deps.send({ type: "force.resume" });
    },
    paused: () => desk.paused,
  };
}

/**
 * What the motor pushed, folded into the desk: a frame paints, a state is the truth.
 *
 * Every message re-arms the watchdog through `show`, which is why a live settle that keeps
 * talking is never declared dead however long it runs.
 */
function absorb(desk: Desk, deps: LiveDeps, publish: () => void): (result: Result) => void {
  return (result) => {
    if (result.type === "force-frame") {
      desk.running = result.frame.running;
      desk.settling = frameBar(result.frame);
      deps.paint(result.frame);
      show(desk, publish);
      return;
    }
    if (result.type === "deltas-structure") {
      // No `show`: a structure snapshot is a drawing, not a claim that a settle is talking, so
      // it must not re-arm the watchdog on its own.
      deps.structure?.(result.run);
      return;
    }
    if (result.type === "force-state") {
      desk.available = result.disabled;
      desk.running = result.running;
      desk.paused = result.paused;
      if (!result.running) desk.settling = HIDDEN;
      show(desk, publish);
    }
  };
}

export function createLiveBridge(deps: LiveDeps): LiveBridge {
  const desk = newDesk();
  const progress = listeners();
  const publish = (): void => progress.set(barOf(desk));
  desk.watchdog = createWatchdog((reason) => dead(desk, deps, publish, reason), {
    schedule: deps.schedule ?? later,
  });
  const onPush = deps.onPush(absorb(desk, deps, publish));
  // A worker that throws reaches the page as an `error` event, and nowhere else: the watchdog
  // and this are the only two ways a live session is ever declared dead.
  const onFail = deps.onFail?.((detail) => desk.watchdog.lost(detail)) ?? (() => undefined);
  const start = (): void => {
    desk.running = true;
    // The strip appears before the first frame, so a settle is never invisible.
    desk.settling = { visible: true, fraction: 1, label: "settling" };
    show(desk, publish);
    deps.send({ type: "force.settle" });
  };
  const destroy = (): void => {
    onPush();
    onFail();
    desk.watchdog.stop();
    deps.send({ type: "force.stop" });
    desk.running = false;
    desk.paused = false;
    desk.busy = 0;
    desk.settling = HIDDEN;
    publish();
  };
  return {
    link: linkOf(desk, deps, publish),
    bar: progress.get,
    onBar: progress.on,
    start,
    batch: (count) => {
      desk.busy = count;
      show(desk, publish);
    },
    destroy,
  };
}
/** What `watchRuns` reads of the studio's state: the calls in flight and the last run drawn. */
export interface RunState {
  readonly busy: readonly unknown[];
  readonly run: { readonly layoutId: string } | null;
}

/**
 * A force layout settles live: the loop steps the session the run left, which keeps the run's
 * picture, or settles a large graph's scatter on screen (`settle.ts`). Every other layout is
 * finished, so nothing starts. A batch layout
 * run shows the same strip with no fraction of its own — one call, no progress inside it.
 *
 * Keyed on the run, not on its layout id: a large graph reports `particle_mesh` whichever force
 * layout was asked for, so two runs in a row can share an id. Keyed on the id, picking
 * `particle_mesh` and then opening 400k nodes left the scatter unsettled (0 frames, 2026-10-03).
 */
export function watchRuns(store: Pick<Store<RunState>, "get" | "subscribe">, bridge: LiveBridge): () => void {
  let seen: RunState["run"] = null;
  let wasBusy = 0;
  return store.subscribe(() => {
    const at = store.get();
    if (at.busy.length !== wasBusy) {
      wasBusy = at.busy.length;
      bridge.batch(wasBusy);
    }
    if (at.run === seen) return;
    seen = at.run;
    if (at.run !== null && settlesLive(at.run.layoutId)) bridge.start();
  });
}
