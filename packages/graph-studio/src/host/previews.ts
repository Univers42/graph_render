/**
 * The previews the hover card and the inspector show: one slot per consumer, at most one host
 * call in flight per slot, and a bounded cache in front of the host (`host-api.md`, verdict 6).
 *
 * Two counters decide whether an answer is still wanted, and each guards its own case:
 * `generation` moves when the graph is replaced, so an answer about the last graph never enters
 * the cache; a slot's `ticket` moves when its consumer moves on or its call times out.
 */
import type { NodePreview, Resolve } from "./contract.ts";
import { type Lru, createLru } from "./lru.ts";
import { PREVIEW_LIMITS, type PreviewLimits, previewOf } from "./preview.ts";

export const CONSUMERS = ["hover", "inspector"] as const;
export type Consumer = (typeof CONSUMERS)[number];

/** What one consumer shows: the id it wants, and the preview once the host gave one. */
export interface Shown {
  readonly id: string | null;
  readonly preview: NodePreview | null;
}

export type Schedule = (run: () => void, ms: number) => () => void;

export interface PreviewDeps {
  /** Read on every call, so a `resolve` the host sets later is the one asked. */
  readonly resolver: () => Resolve | null;
  readonly schedule?: Schedule;
  readonly capacity?: number;
  readonly limits?: PreviewLimits;
  readonly timeoutMs?: number;
  readonly delays?: Readonly<Record<Consumer, number>>;
}

export interface Previews {
  want(consumer: Consumer, id: string | null): void;
  /** The same object until it changes, for `useSyncExternalStore`. */
  shown(consumer: Consumer): Shown;
  subscribe(listener: () => void): () => void;
  invalidate(id: string): void;
  /** A new graph: the cache empties and nothing asked about the old one is kept. */
  clear(): void;
}

export const PREVIEW_CAPACITY = 256;
/**
 * Caveat: a host that takes longer than this is cut off and its answer dropped, even when it
 * would have been right; one that ignores `signal` keeps working after the cut.
 */
export const RESOLVE_TIMEOUT_MS = 5000;
/** Caveat: a pointer that rests on a node for less than 150 ms never asks the host at all. */
export const DELAYS: Readonly<Record<Consumer, number>> = Object.freeze({ hover: 150, inspector: 0 });

interface Slot {
  shown: Shown;
  ticket: number;
  /** Cancels the debounce or aborts the call in flight. */
  stop: () => void;
}

interface Desk {
  readonly deps: Required<PreviewDeps>;
  readonly cache: Lru<NodePreview>;
  readonly slots: Record<Consumer, Slot>;
  readonly listeners: Set<() => void>;
  generation: number;
}

const IDLE: Shown = Object.freeze({ id: null, preview: null });
const NOTHING = (): void => undefined;

function timer(run: () => void, ms: number): () => void {
  const handle = setTimeout(run, ms);
  return () => clearTimeout(handle);
}

function notify(desk: Desk): void {
  for (const listener of [...desk.listeners]) listener();
}

function settle(desk: Desk, id: string, preview: NodePreview): void {
  desk.cache.set(id, preview);
  for (const consumer of CONSUMERS) {
    const slot = desk.slots[consumer];
    if (slot.shown.id === id) slot.shown = Object.freeze({ id, preview });
  }
  notify(desk);
}

function call(desk: Desk, slot: Slot, id: string): void {
  const resolve = desk.deps.resolver();
  if (resolve === null) return;
  const controller = new AbortController();
  const [generation, ticket] = [desk.generation, slot.ticket];
  const giveUp = (): void => {
    if (slot.ticket === ticket) slot.ticket += 1;
    controller.abort();
  };
  const cancelTimeout = desk.deps.schedule(giveUp, desk.deps.timeoutMs);
  slot.stop = () => {
    cancelTimeout();
    controller.abort();
  };
  const answered = (value: unknown): void => {
    cancelTimeout();
    if (generation !== desk.generation || ticket !== slot.ticket) return;
    slot.stop = NOTHING;
    const preview = previewOf(value, desk.deps.limits);
    if (preview !== null) settle(desk, id, preview);
  };
  void Promise.resolve().then(() => resolve(id, controller.signal)).then(answered, cancelTimeout);
}

function want(desk: Desk, consumer: Consumer, id: string | null): void {
  const slot = desk.slots[consumer];
  if (slot.shown.id === id) return;
  slot.stop();
  slot.stop = NOTHING;
  slot.ticket += 1;
  const cached = id === null ? undefined : desk.cache.get(id);
  slot.shown = id === null ? IDLE : Object.freeze({ id, preview: cached ?? null });
  notify(desk);
  if (id === null || cached !== undefined || desk.deps.resolver() === null) return;
  slot.stop = desk.deps.schedule(() => call(desk, slot, id), desk.deps.delays[consumer]);
}

function invalidate(desk: Desk, id: string): void {
  desk.cache.delete(id);
  for (const consumer of CONSUMERS) {
    const slot = desk.slots[consumer];
    if (slot.shown.id !== id) continue;
    slot.stop();
    slot.ticket += 1;
    slot.shown = Object.freeze({ id, preview: null });
    call(desk, slot, id);
  }
  notify(desk);
}

/**
 * WHY `clear` aborts but leaves the tickets: the abort tells the host to stop working, and the
 * generation is what keeps a host that answers anyway out of the new graph's cache.
 */
function clear(desk: Desk): void {
  desk.generation += 1;
  desk.cache.clear();
  for (const consumer of CONSUMERS) {
    const slot = desk.slots[consumer];
    slot.stop();
    slot.stop = NOTHING;
    slot.shown = IDLE;
  }
  notify(desk);
}

export function createPreviews(deps: PreviewDeps): Previews {
  const full: Required<PreviewDeps> = {
    schedule: timer, capacity: PREVIEW_CAPACITY, limits: PREVIEW_LIMITS, timeoutMs: RESOLVE_TIMEOUT_MS, delays: DELAYS, ...deps,
  };
  const slot = (): Slot => ({ shown: IDLE, ticket: 0, stop: NOTHING });
  const desk: Desk = {
    deps: full, cache: createLru(full.capacity), slots: { hover: slot(), inspector: slot() }, listeners: new Set(), generation: 0,
  };
  return {
    want: (consumer, id) => want(desk, consumer, id),
    shown: (consumer) => desk.slots[consumer].shown,
    subscribe: (listener) => {
      desk.listeners.add(listener);
      return () => desk.listeners.delete(listener);
    },
    invalidate: (id) => invalidate(desk, id),
    clear: () => clear(desk),
  };
}
