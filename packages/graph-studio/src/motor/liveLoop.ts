/**
 * The worker's force host: the port the page asks through, and the loop it drives. The port is
 * asked for on every request rather than handed over, because the motor's session exists only
 * once a graph has been loaded and laid out, and the host is made before either. `null` from
 * the port is the no-adapter answer, with the reason.
 */
import { DeltaRefusal } from "./deltas.ts";
import { type LiveForce, NO_ADAPTER_REASON } from "./live.ts";
import { ForceLoop } from "./loop.ts";
import type { ForceRequest, GraphBatch, Result, RunReport } from "./protocol.ts";
import { describeError } from "../state/errors.ts";
import type { GrowMark } from "./deltas.ts";

export { ALPHA_MIN, DEFAULT_PERIOD_MS, PAUSED_REASON, TICKS_PER_FRAME } from "./loop.ts";

export interface LoopDeps {
  /** Calls `run` once after `delayMs`; returns a cancel. Never calls twice for one call. */
  readonly schedule: (run: () => void, delayMs: number) => () => void;
  readonly now: () => number;
  readonly emit: (result: Result, transfer: ArrayBufferLike[]) => void;
  readonly periodMs?: number;
  /**
   * Rebuilds the structure snapshot a delta batch needs drawn, throttled by the queue. Absent
   * where there is no session to grow — the batch still answers, and the page keeps the node
   * count it has.
   */
  readonly structure?: () => Promise<RunReport | null>;
}

export interface ForceHost {
  readonly handle: (request: ForceRequest) => Result;
  /**
   * Queues one batch of nodes and edges and answers with what the tick that applied it made of
   * it. One answer per call, refused whole or applied whole; the extends themselves are
   * coalesced per tick with one grow between them.
   */
  deltas(batch: GraphBatch): Promise<Result>;
  /** The grows the queue has done, oldest first, at most 1024. */
  grows(): readonly GrowMark[];
  /**
   * The session the loop is ticking is gone: stop at once, without waiting for the next
   * request. A graph replaced mid-settle releases its force session, and the frame already
   * scheduled would step a session the motor has already thrown away.
   */
  forget(): void;
}

export function createForceHost(port: () => LiveForce | null, deps: LoopDeps): ForceHost {
  // The loop is made on the first request that finds a port, and released with it: a
  // re-layout makes a new session, so the loop must not keep ticking on the old one.
  let loop: { readonly loop: ForceLoop; readonly port: LiveForce } | null = null;
  // `release`, not `halt`: nothing asked for this stop, so the loop has to say how it ended or
  // the page's watchdog reads the quiet as a dead worker (see ForceLoop.release).
  const forget = (): void => {
    loop?.loop.release();
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
    // No loop means no session, and no session means a batch has nothing to be applied to.
    deltas: async (batch) => {
      const running = live();
      return running === null
        ? { type: "failed", error: describeError(new DeltaRefusal(NO_ADAPTER_REASON)) }
        : running.deltas(batch);
    },
    grows: () => loop?.loop.grows() ?? [],
    forget,
  };
}
