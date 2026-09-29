/**
 * The worker's stepping loop over a `LiveForce`: one bounded batch per frame, one frame out,
 * and no backlog. It stops on its own once alpha is under `alphaMin` and no pin is held.
 */
import type { ForceRequest, Result } from "./protocol.ts";
import { type LiveForce, NO_ADAPTER_REASON } from "./live.ts";

/**
 * Ponytail: the budget is wall time from `now`, checked between ticks, so one tick that
 * itself overruns (a very large graph) still overruns the frame; only frames after it are
 * skipped. The 8 ms default is a guess at half a 60 Hz frame, not a measurement.
 */
export const DEFAULT_BUDGET_MS = 8;
export const ALPHA_MIN = 0.001;
const REHEAT_ALPHA = 0.3;
const TICKS_PER_BATCH = 4;

export interface LoopDeps {
  /** Calls `run` once at the next frame; returns a cancel. Never calls twice for one call. */
  readonly schedule: (run: () => void) => () => void;
  readonly now: () => number;
  readonly emit: (result: Result, transfer: ArrayBufferLike[]) => void;
  readonly budgetMs?: number;
}

export interface ForceHost {
  readonly handle: (request: ForceRequest) => Result;
}

interface Pin { readonly x: number; readonly y: number }

class ForceLoop {
  private readonly held = new Set<string>();
  // Latest pointer position per node: a move that arrives before the frame replaces the
  // last one, which is how a slow motor drops drag events instead of queueing them.
  private readonly pending = new Map<string, Pin>();
  private cancel: (() => void) | null = null;
  private alpha = 0;

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

  private batch(): void {
    const budget = this.deps.budgetMs ?? DEFAULT_BUDGET_MS;
    const began = this.deps.now();
    do {
      this.alpha = this.live.step(TICKS_PER_BATCH);
    } while (this.alpha >= ALPHA_MIN && this.deps.now() - began < budget);
  }

  private frame(): void {
    this.cancel = null;
    for (const [id, pin] of this.pending) this.live.pin(id, pin.x, pin.y);
    this.pending.clear();
    this.batch();
    const running = this.alpha >= ALPHA_MIN || this.held.size > 0;
    const { xs, ys } = this.live.positions();
    const copy = { xs: xs.slice(), ys: ys.slice(), alpha: this.alpha, running };
    this.deps.emit({ type: "force-frame", frame: copy }, [copy.xs.buffer, copy.ys.buffer]);
    if (running) this.cancel = this.deps.schedule(() => this.frame());
  }

  private wake(): void {
    this.live.reheat(REHEAT_ALPHA);
    this.alpha = Math.max(this.alpha, REHEAT_ALPHA);
    this.cancel ??= this.deps.schedule(() => this.frame());
  }

  private halt(): void {
    this.cancel?.();
    this.cancel = null;
    for (const id of this.held) this.live.unpin(id);
    this.held.clear();
    this.pending.clear();
  }

  apply(request: ForceRequest): void {
    if (request.type === "force.stop") {
      this.halt();
      return;
    }
    if (request.type === "force.drag") {
      this.held.add(request.id);
      this.pending.set(request.id, { x: request.x, y: request.y });
    } else if (request.type === "force.release") {
      this.pending.delete(request.id);
      this.held.delete(request.id);
      this.live.unpin(request.id);
    } else if (request.type === "force.params") this.live.setParams(request.knobs);
    this.wake();
  }
}

export function createForceHost(port: LiveForce | null, deps: LoopDeps): ForceHost {
  const loop = port === null ? null : new ForceLoop(port, deps);
  return {
    handle(request) {
      loop?.apply(request);
      return { type: "force-state", running: loop?.running ?? false, disabled: port === null ? NO_ADAPTER_REASON : null };
    },
  };
}
