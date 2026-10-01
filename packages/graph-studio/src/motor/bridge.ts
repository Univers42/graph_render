/**
 * The page's side of the live loop, in one object: the `ForceLink` the forces actions and
 * the drag reach, and the store the progress bar reads.
 *
 * The frames arrive here, not in the studio's state: a settle pushes one per animation
 * frame, and putting that in the store every panel reads would re-render the whole chrome
 * sixty times a second. So the bar has its own store with its own subscribers, and the
 * positions go straight to the view, which redraws on its own request.
 *
 * Ponytail: `disabled()` reads the last state the loop reported, so the panel is greyed
 * until the worker has answered one force request and knows whether it has a session.
 * Failing input: a worker that dies between frames leaves the last state standing, and the
 * panel goes on believing a loop is running; `forces.animate` is what rediscovers it.
 * Direction: the state is the worker's, not a guess made here. Escape hatch: `start` is
 * called after every force layout, so a re-layout makes a new session and a new loop.
 */
import type { ForceLink } from "../actions/forces.ts";
import { DEFAULT_KNOBS, type ForceKnobs } from "./live.ts";
import type { ForceFrame, ForceRequest, Result } from "./protocol.ts";
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

/**
 * Ponytail: which layouts settle live is read off the id (`layout.force…`) rather than kept
 * in a table, so a force engine registered under a name nobody predicted still settles on
 * screen. Failing input: `layout.random` says nothing about force and is excluded by that
 * read, so the random layout stays a finished picture. Direction: substring, so
 * `layout.forceatlas2` and a future `layout.force.barnes_hut` both qualify.
 */
export function settlesLive(layoutId: string): boolean {
  return layoutId.startsWith("layout.force");
}

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
}

export const NOT_ASKED = "the motor has not been asked yet";

function newDesk(): Desk {
  return { knobs: DEFAULT_KNOBS, running: false, paused: false, available: undefined, settling: HIDDEN, busy: 0 };
}

/** One strip, two things that can want it: the settle wins while it runs. */
function barOf(desk: Desk): Bar {
  return desk.busy > 0 && !desk.running ? batchBar(desk.busy) : desk.settling;
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
    set: (next) => {
      desk.knobs = next;
      deps.send({ type: "force.params", knobs: next });
    },
    animate: (on) => {
      desk.running = on;
      deps.send({ type: "force.start" });
      if (on) return;
      deps.send({ type: "force.stop" });
      desk.settling = HIDDEN;
      publish();
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

/** What the motor pushed, folded into the desk: a frame paints, a state is the truth. */
function absorb(desk: Desk, deps: LiveDeps, publish: () => void): (result: Result) => void {
  return (result) => {
    if (result.type === "force-frame") {
      desk.running = result.frame.running;
      desk.settling = frameBar(result.frame);
      deps.paint(result.frame);
      publish();
      return;
    }
    if (result.type === "force-state") {
      desk.available = result.disabled;
      desk.running = result.running;
      desk.paused = result.paused;
      if (!result.running) desk.settling = HIDDEN;
      publish();
    }
  };
}

export function createLiveBridge(deps: LiveDeps): LiveBridge {
  const desk = newDesk();
  const progress = listeners();
  const publish = (): void => progress.set(barOf(desk));
  const onPush = deps.onPush(absorb(desk, deps, publish));
  const start = (): void => {
    desk.running = true;
    // The strip appears before the first frame, so a settle is never invisible.
    desk.settling = { visible: true, fraction: 1, label: "settling" };
    publish();
    deps.send({ type: "force.start" });
  };
  const destroy = (): void => {
    onPush();
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
      publish();
    },
    destroy,
  };
}