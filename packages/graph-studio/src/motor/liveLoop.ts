/**
 * The worker's stepping loop over a `LiveForce`: one bounded batch per frame, one frame out,
 * and no backlog. It stops on its own once alpha is under `alphaMin` and no pin is held.
 */
import type { ForceRequest, Result } from "./protocol.ts";
import { type LiveForce, NO_ADAPTER_REASON } from "./live.ts";

/**
 * Ponytail: the budget is wall time from `now`, measured around the one batch, and a batch
 * that overran it has its NEXT tick dropped rather than run late. The default 8 ms is half a
 * 60 Hz frame: a guess, not a measurement.
 *
 * The frame rate paces the simulation, not the batch size: one tick a frame makes the motor's
 * own 112-tick settle last about two seconds, which is a settle a person can watch. The
 * budget is a ceiling, never a target — a loop that filled it with more ticks would finish
 * the whole settle inside ten frames and there would be nothing on screen to look at.
 */
export const DEFAULT_BUDGET_MS = 8;
export const ALPHA_MIN = 0.001;
const REHEAT_ALPHA = 0.3;
/** One tick a frame. The motor's frozen settle is 112 ticks (`ForceParams::TICKS`). */
export const TICKS_PER_FRAME = 1;

export interface LoopDeps {
  /** Calls `run` once at the next frame; returns a cancel. Never calls twice for one call. */
  readonly schedule: (run: () => void) => () => void;
  readonly now: () => number;
  readonly emit: (result: Result, transfer: ArrayBufferLike[]) => void;
  readonly budgetMs?: number;
}

export interface ForceHost {
  readonly handle: (request: ForceRequest) => Result;
  /**
   * The session the loop is ticking is gone: stop at once, without waiting for the next
   * request. A graph replaced mid-settle releases its force session, and the frame already
   * scheduled would step a session the motor has already thrown away.
   */
  forget(): void;
}

interface Pin { readonly x: number; readonly y: number }

class ForceLoop {
  private readonly held = new Set<string>();
  // Where the loop's pins are: the latest pointer position per node, a move that arrives
  // before the frame replacing the last one, which is how a slow motor drops drag events
  // instead of queueing them. An id stays in here after its release — a dropped node is a node
  // the user put somewhere on purpose — so the pin outlives the pointer that made it.
  private readonly pinned = new Map<string, Pin>();
  private cancel: (() => void) | null = null;
  private alpha = 0;
  private paused = false;
  /** True when the last tick overran its budget, so this frame draws without stepping. */
  private dropped = false;

  private readonly live: LiveForce;
  private readonly deps: LoopDeps;

  // WHY fields and not parameter properties: the tests run under node's type stripping, which refuses them.
  constructor(live: LiveForce, deps: LoopDeps) {
    this.live = live;
    this.deps = deps;
  }

  get running(): boolean {
    return this.cancel !== null;
  }

  get isPaused(): boolean {
    return this.paused;
  }

  private batch(): void {
    const budget = this.deps.budgetMs ?? DEFAULT_BUDGET_MS;
    const began = this.deps.now();
    this.alpha = this.live.step(TICKS_PER_FRAME);
    // One tick late is one tick too many: the next frame draws without stepping, so the
    // simulation loses a tick rather than the page losing a frame.
    this.dropped = this.deps.now() - began >= budget;
  }

  private frame(): void {
    this.cancel = null;
    // WHY this is first: the port can die between the request that scheduled this frame and
    // the frame itself, and every call on a released session throws. There is nothing to
    // step, nothing to draw and nothing left to schedule, so the frame just ends.
    if (this.live.dead === true) {
      this.held.clear();
      this.pinned.clear();
      return;
    }
    for (const [id, at] of this.pinned) this.live.pin(id, at.x, at.y);
    if (this.dropped) this.dropped = false;
    else this.batch();
    const running = this.alpha >= ALPHA_MIN || this.held.size > 0;
    const { xs, ys } = this.live.positions();
    const copy = { xs: xs.slice(), ys: ys.slice(), alpha: this.alpha, running };
    this.deps.emit({ type: "force-frame", frame: copy }, [copy.xs.buffer, copy.ys.buffer]);
    if (running) this.cancel = this.deps.schedule(() => this.frame());
  }

  private wake(): void {
    // The reheated alpha is the larger of the two: a port that just restarted from random
    // positions is at the top, and reheating it back down to REHEAT_ALPHA would restart the
    // settle from a quarter-settled drawing.
    const heated = Math.max(REHEAT_ALPHA, this.alpha);
    this.live.reheat(heated);
    this.alpha = heated;
    this.cancel ??= this.deps.schedule(() => this.frame());
  }

  /**
   * Stops scheduling but keeps the pins and the alpha reached, so the graph is drawn where
   * it was frozen. `resume` carries on from there instead of reheating: a pause is not a
   * reset, and reheating on resume would restart the settle the user paused.
   */
  pause(): void {
    this.paused = true;
    this.cancel?.();
    this.cancel = null;
    this.publish(false);
  }

  resume(): void {
    if (!this.paused) return;
    this.paused = false;
    this.cancel ??= this.deps.schedule(() => this.frame());
  }

  halt(): void {
    this.paused = false;
    this.cancel?.();
    this.cancel = null;
    this.drop();
  }

  /** Lets go of every pin the loop owns, dropped nodes included. */
  private drop(): void {
    // A released session throws from an unpin too, and there is no pin left on it to lift.
    if (this.live.dead !== true) for (const id of this.pinned.keys()) this.live.unpin(id);
    this.held.clear();
    this.pinned.clear();
  }

  /** One last frame saying the loop has stopped, so the bar empties instead of hanging. */
  private publish(running: boolean): void {
    const { xs, ys } = this.live.positions();
    const copy = { xs: xs.slice(), ys: ys.slice(), alpha: this.alpha, running };
    this.deps.emit({ type: "force-frame", frame: copy }, [copy.xs.buffer, copy.ys.buffer]);
  }

  apply(request: ForceRequest): void {
    if (request.type === "force.stop") {
      this.halt();
      return;
    }
    if (request.type === "force.pause") {
      this.pause();
      return;
    }
    if (request.type === "force.resume") {
      this.resume();
      return;
    }
    // A drag wakes the loop even from a pause: the user is holding the graph, and a drawing
    // frozen under the pointer is not what a pause was for.
    this.paused = false;
    if (request.type === "force.start") {
      // "Animate": the settle starts over from random positions, not from where it stopped.
      // The port answers with the alpha it re-heated to, which is the bar's new full width.
      // A dropped node is a position the user chose, and a restart throws the nodes back to
      // random ones — so the pins go with the positions they were holding.
      this.drop();
      const restarted = this.live.shuffle?.();
      if (restarted !== undefined) this.alpha = restarted;
    } else if (request.type === "force.drag") {
      this.held.add(request.id);
      this.pinned.set(request.id, { x: request.x, y: request.y });
    } else if (request.type === "force.release") {
      // WHY the pin stays: the motor integrates a released node from rest and the drawing
      // settles it straight back to the equilibrium the drag just broke, so lifting the pin
      // here makes a drop undo itself. The node keeps the position the user put it at; the
      // graph settles around it. `held` is what the pointer is holding, and only that keeps
      // the loop awake — so a drop lets the settle finish.
      this.held.delete(request.id);
    } else this.live.setParams(request.knobs);
    this.wake();
  }
}

/**
 * The worker's force host. The port is asked for on every request rather than handed over,
 * because the motor's session exists only once a graph has been loaded and laid out, and the
 * host is made before either. `null` from the port is the no-adapter answer, with the reason.
 */
export function createForceHost(port: () => LiveForce | null, deps: LoopDeps): ForceHost {
  // The loop is made on the first request that finds a port, and released with it: a
  // re-layout makes a new session, so the loop must not keep ticking on the old one.
  let loop: { readonly loop: ForceLoop; readonly port: LiveForce } | null = null;
  const forget = (): void => {
    loop?.loop.halt();
    loop = null;
  };
  const live = (): ForceLoop | null => {
    const found = port();
    if (found === null) {
      forget();
      return null;
    }
    if (loop !== null && loop.port !== found) forget();
    loop ??= { loop: new ForceLoop(found, deps), port: found };
    return loop.loop;
  };
  return {
    handle(request) {
      const running = live();
      if (running === null) return { type: "force-state", running: false, disabled: NO_ADAPTER_REASON, paused: false };
      running.apply(request);
      return { type: "force-state", running: running.running, disabled: null, paused: running.isPaused };
    },
    forget,
  };
}
