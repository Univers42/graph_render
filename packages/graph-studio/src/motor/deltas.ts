/**
 * The delta queue. A host hands over one batch at a time and each is applied whole, but the
 * work is coalesced per tick: every batch queued since the last tick is extended first, then
 * the live session is grown once and reheated once for the whole burst. So ten calls in one
 * animation frame cost ten extends, one grow and one reheat — and ten answers, one per call,
 * because a call either succeeds whole or is refused whole.
 *
 * `docs/contract/delta.md` is the contract. The two rules it and `host-api.md` between them
 * both bind are kept: atomic per call (a batch's `applied` is every one of its nodes or none)
 * and one grow and one reheat per tick for every batch queued since the last one.
 */
import { type ShownError, describeError } from "../state/errors.ts";
import type { GraphBatch, Result, RunReport } from "./protocol.ts";

/** One applied batch, as `(tick, batch)`: the tick that grew for it, and its place in the burst. */
export interface GrowMark {
  readonly tick: number;
  readonly batch: number;
}

/** The alpha a grown session is put back to; a session cooler than this is reheated to it. */
export const GROW_ALPHA = 0.3;

/** A structure snapshot is rebuilt at most this often — twice a second, never more. */
export const STRUCTURE_MS = 500;

/** How many grows are remembered. A replay reads the tail of a session, not all of it. */
export const RING_LIMIT = 1024;

export interface QueueDeps {
  /** One batch into the built graph; throws and the graph is as it was. */
  readonly extend?: (batch: GraphBatch) => void;
  /** Covers the graph's new node count in the live session. Throws when it cannot. */
  readonly grow?: () => void;
  readonly reheat: (alpha: number) => void;
  /** The session's alpha now, so the reheat is the larger of it and `GROW_ALPHA`. */
  readonly alpha: () => number;
  /** How many nodes the session covers, read after a grow. */
  readonly nodeCount: () => number;
  /** Rebuilds the structure snapshot the page draws; `null` when the motor refuses it. */
  readonly structure?: () => Promise<RunReport | null>;
  readonly emit: (result: Result, transfer: ArrayBufferLike[]) => void;
  readonly now: () => number;
}

export interface DeltaQueue {
  /** Queues one batch and answers with what the tick that applied it made of it. */
  readonly push: (batch: GraphBatch) => Promise<Result>;
  /** Applies everything queued, in arrival order; returns once the answers are given. */
  readonly drain: (tick: number) => Promise<void>;
  /** Refuses everything still queued: the session they were for is gone. */
  readonly refuse: (message: string) => void;
  /** The last `RING_LIMIT` grows, oldest first. */
  readonly grows: () => readonly GrowMark[];
  /** How many structure snapshots this queue has rebuilt. */
  readonly rebuilds: () => number;
}

interface Queued {
  readonly batch: GraphBatch;
  readonly answer: (result: Result) => void;
}

/** What a port with no extend or grow path refuses a batch with; `describeError` keeps the name. */
export class DeltaRefusal extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DeltaRefusal";
  }
}

/** Why a batch cannot be applied at all, or null when it can. */
const NO_PATH = "this motor cannot add to a built graph, or its live session cannot grow";

interface Applied {
  readonly queued: Queued;
  readonly applied: number;
}

interface Refused {
  readonly queued: Queued;
  readonly error: ShownError;
}

export function createDeltaQueue(deps: QueueDeps): DeltaQueue {
  const pending: Queued[] = [];
  const ring: GrowMark[] = [];
  let rebuilds = 0;
  let lastStructure = Number.NEGATIVE_INFINITY;
  /** Whether the tick before this one found nothing queued: the burst that just landed is over. */
  let drainedLast = true;
  let building = false;

  const extendAll = (burst: readonly Queued[]): (Applied | Refused)[] => burst.map((queued) => {
    try {
      deps.extend?.(queued.batch);
      return { queued, applied: queued.batch.nodes.length };
    } catch (error) {
      return { queued, error: describeError(error) };
    }
  });

  const answer = (one: Applied | Refused, nodeCount: number): void => {
    one.queued.answer("error" in one
      ? { type: "failed", error: one.error }
      : { type: "deltas-applied", applied: one.applied, nodeCount });
  };

  /**
   * Why a rebuild is due now: the cadence has come round, or the burst that just landed is the
   * last one — nothing was queued for the tick before, so nothing more is on its way and the
   * nodes can be drawn at once instead of waiting out the cadence.
   */
  const due = (landed: number): boolean => landed > 0
    && (deps.now() - lastStructure >= STRUCTURE_MS || drainedLast);

  async function rebuild(): Promise<void> {
    if (deps.structure === undefined || building) return;
    building = true;
    lastStructure = deps.now();
    rebuilds += 1;
    try {
      const run = await deps.structure();
      if (run !== null) deps.emit({ type: "deltas-structure", run }, [run.bytes.buffer]);
    } catch {
      // A refused rebuild leaves the snapshot the page already has; the next batch tries again.
    } finally {
      building = false;
    }
  }

  /**
   * One grow and one reheat for the whole burst, then one answer per batch.
   *
   * Caveat: `nodeCount` is the count after the whole burst, so every batch in one burst answers
   * the same count; a per-batch count would have to predict a grow that has not run yet.
   * Failing input: a refused batch in the middle changes nothing — the other batches still go
   * in and the grow still runs, because the graph did change. Direction: the reheat is
   * `max(alpha, GROW_ALPHA)`, so a port that was just reheated is not cooled back down.
   * Escape hatch: `structure` left out means no snapshot is rebuilt and the page keeps drawing
   * the node count it has, so the nodes move in the motor and are not drawn.
   */
  async function drain(tick: number): Promise<void> {
    const burst = pending.splice(0, pending.length);
    const landed = burst.length;
    drainedLast = landed === 0;
    if (landed === 0) return;
    const applied = extendAll(burst);
    let any = false;
    for (const one of applied) {
      if ("error" in one) continue;
      any = true;
      // Bounded here rather than at the reader: a session that runs for hours must not keep a
      // mark per grow forever, and 1024 is a ring, not a growing log.
      if (ring.length >= RING_LIMIT) ring.shift();
      ring.push({ tick, batch: applied.indexOf(one) });
    }
    if (any) {
      deps.grow?.();
      deps.reheat(Math.max(GROW_ALPHA, deps.alpha()));
    }
    const count = deps.nodeCount();
    for (const one of applied) answer(one, count);
    if (any && due(landed)) await rebuild();
  }

  return {
    // Refused here rather than at the drain: a port with no extend path has nothing to apply a
    // batch to, and queueing it would answer a tick later with a grow that cannot happen.
    push: (batch) => deps.extend === undefined || deps.grow === undefined
      ? Promise.resolve({ type: "failed", error: describeError(new DeltaRefusal(NO_PATH)) })
      : new Promise<Result>((resolve) => { pending.push({ batch, answer: resolve }); }),
    drain,
    refuse: (message) => {
      const stopped = pending.splice(0, pending.length);
      for (const one of stopped) one.answer({ type: "failed", error: describeError(new Error(message)) });
    },
    grows: () => ring.slice(),
    rebuilds: () => rebuilds,
  };
}
