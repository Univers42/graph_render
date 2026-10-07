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
  /**
   * The grow still on its way into the session, or null once it is in. The GPU arm's grow waits
   * for the batch in flight (`gpuPort.ts`); the node count is read after it. Absent: synchronous.
   */
  readonly settled?: () => Promise<void> | null;
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

/**
 * The structure snapshot's cadence: at most twice a second, and at once when the tick before
 * this one found nothing queued — that is the burst being over, and there is nothing to wait for.
 */
interface Cadence {
  /** A batch went in and nothing has drawn it yet: a rebuild is owed. */
  readonly owe: () => void;
  /** Runs the rebuild if one is due; `wasIdle` says the tick before this one was empty. */
  readonly run: (wasIdle: boolean) => Promise<void>;
  readonly count: () => number;
}

function cadence(deps: QueueDeps): Cadence {
  let done = 0;
  let last = Number.NEGATIVE_INFINITY;
  let owed = false;
  let building = false;
  const push = (run: RunReport): void => deps.emit({ type: "deltas-structure", run }, [run.bytes.buffer]);
  return {
    owe: () => { owed = true; },
    run: async (wasIdle) => {
      // Owed, and either the tick before was empty (the burst is over, so draw it now) or the
      // cadence has come round. Every frame asks, which is what draws a burst's last batch after
      // the host has stopped calling: the queue is only drained by a tick, and nothing is queued.
      if (deps.structure === undefined || building || !owed) return;
      if (!wasIdle && deps.now() - last < STRUCTURE_MS) return;
      building = true;
      owed = false;
      last = deps.now();
      done += 1;
      try {
        const run = await deps.structure();
        if (run !== null) push(run);
      } catch {
        // A refused rebuild leaves the snapshot the page already has; the next batch tries again.
      } finally {
        building = false;
      }
    },
    count: () => done,
  };
}

/** One extend per batch, in arrival order: a refusal is the batch's own answer. */
function extendAll(deps: QueueDeps, burst: readonly Queued[]): (Applied | Refused)[] {
  return burst.map((queued) => {
    try {
      deps.extend?.(queued.batch);
      return { queued, applied: queued.batch.nodes.length };
    } catch (error) {
      return { queued, error: describeError(error) };
    }
  });
}

/**
 * One grow and one reheat for the whole burst, and the node count the session covers after it.
 *
 * Caveat: a grow that throws leaves the batch in the graph and the session short of it, so the
 * count is the one from before and the answer still says the nodes went in — which happened.
 * Direction: the reheat is `max(alpha, GROW_ALPHA)`, so a port just reheated is not cooled down.
 * Escape hatch: `grow` left out answers with the count the session had.
 */
function growFor(deps: QueueDeps): number {
  const read = (): number => {
    try {
      return deps.nodeCount();
    } catch {
      return 0;
    }
  };
  try {
    deps.grow?.();
    deps.reheat(Math.max(GROW_ALPHA, deps.alpha()));
    return deps.nodeCount();
  } catch {
    return read();
  }
}

/** The count once an asynchronous grow is in; a grow that fails keeps `grown`, as above. */
async function settledCount(deps: QueueDeps, growing: Promise<void>, grown: number): Promise<number> {
  try {
    await growing;
    return deps.nodeCount();
  } catch {
    return grown;
  }
}

/** The ring's marks for the burst, in order, and the count of batches that went in. */
function mark(applied: readonly (Applied | Refused)[], ring: GrowMark[], tick: number): number {
  let any = 0;
  for (const one of applied) {
    if ("error" in one) continue;
    any += 1;
    // Bounded here rather than at the reader: a session that runs for hours must not keep a
    // mark per grow forever, and 1024 is a ring, not a growing log.
    if (ring.length >= RING_LIMIT) ring.shift();
    ring.push({ tick, batch: applied.indexOf(one) });
  }
  return any;
}

export function createDeltaQueue(deps: QueueDeps): DeltaQueue {
  const pending: Queued[] = [];
  const ring: GrowMark[] = [];
  /** Whether the tick before this one found nothing queued: the burst that just landed is over. */
  let drainedLast = true;
  const snapshots = cadence(deps);

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
    // Read before it is written: what the cadence wants to know is whether the tick BEFORE this
    // one found nothing, which is what says this burst is the last one.
    const wasIdle = drainedLast;
    drainedLast = burst.length === 0;
    if (burst.length === 0) {
      await snapshots.run(wasIdle);
      return;
    }
    const applied = extendAll(deps, burst);
    const any = mark(applied, ring, tick);
    const grown = any > 0 ? growFor(deps) : deps.nodeCount();
    // Awaited only when a grow is still on its way: the CPU arm answers without a microtask.
    const growing = any > 0 ? deps.settled?.() ?? null : null;
    const count = growing === null ? grown : await settledCount(deps, growing, grown);
    for (const one of applied) {
      one.queued.answer("error" in one
        ? { type: "failed", error: one.error }
        : { type: "deltas-applied", applied: one.applied, nodeCount: count });
    }
    if (any > 0) snapshots.owe();
    await snapshots.run(wasIdle);
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
    rebuilds: () => snapshots.count(),
  };
}
