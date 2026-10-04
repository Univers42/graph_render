/**
 * `applyDeltas`, the one verb a host hands a whole batch to. Its batch is checked against the
 * same refusal `resolve` uses (`registry.ts`), so a bad member reads like every other bad value.
 */
import { ActionRefusal, bad } from "./registry.ts";
import { EDGE_KINDS, NODE_KINDS } from "../source/ingest.ts";
import type { DeltaEdge, DeltaNode, GraphBatch } from "../motor/protocol.ts";

/** The verb a host feature-tests with (`"applyDeltas" in el`, host-api condition 8). */
export const APPLY_DELTAS = "applyDeltas";

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function text(record: Readonly<Record<string, unknown>>, name: string, at: string): string {
  const value = record[name];
  if (typeof value !== "string") throw bad(`${at}.${name}`, `must be text, not ${JSON.stringify(value ?? null)}`);
  return value;
}

function maybeText(record: Readonly<Record<string, unknown>>, name: string, at: string): string | null {
  const value = record[name];
  if (value === null) return null;
  if (typeof value !== "string") throw bad(`${at}.${name}`, `must be text or null, not ${JSON.stringify(value)}`);
  return value;
}

function kind<Kind extends string>(record: Readonly<Record<string, unknown>>, name: string, at: string, kinds: readonly Kind[]): Kind {
  const value = text(record, name, at);
  const found = kinds.find((one) => one === value);
  if (found === undefined) throw bad(`${at}.${name}`, `must be one of ${kinds.join(", ")}, not ${value}`);
  return found;
}

function count(record: Readonly<Record<string, unknown>>, name: string, at: string): number {
  const value = record[name];
  if (typeof value !== "number" || !Number.isFinite(value)) throw bad(`${at}.${name}`, `must be a number, not ${JSON.stringify(value ?? null)}`);
  return value;
}

function flag(record: Readonly<Record<string, unknown>>, name: string, at: string): boolean {
  const value = record[name];
  if (typeof value !== "boolean") throw bad(`${at}.${name}`, `must be on or off, not ${JSON.stringify(value ?? null)}`);
  return value;
}

function nodeOf(value: unknown, at: string): DeltaNode {
  if (!isRecord(value)) throw bad(at, "must be objects");
  const record = value;
  return {
    id: text(record, "id", at), kind: kind(record, "kind", at, NODE_KINDS), database_id: maybeText(record, "database_id", at),
    source: text(record, "source", at), label: text(record, "label", at), group: maybeText(record, "group", at),
    weight: count(record, "weight", at), version: count(record, "version", at),
    has_note: flag(record, "has_note", at), icon: maybeText(record, "icon", at),
  };
}

function edgeOf(value: unknown, at: string): DeltaEdge {
  if (!isRecord(value)) throw bad(at, "must be objects");
  const record = value;
  return {
    id: text(record, "id", at), source: text(record, "source", at), target: text(record, "target", at),
    kind: kind(record, "kind", at, EDGE_KINDS), label: text(record, "label", at), strength: count(record, "strength", at),
    directed: flag(record, "directed", at), record_id: maybeText(record, "record_id", at),
    child_first: flag(record, "child_first", at),
  };
}

function entriesOf<T extends DeltaNode | DeltaEdge>(value: unknown, name: string, of: (entry: unknown, at: string) => T): readonly T[] {
  if (!Array.isArray(value)) throw bad(name, `must be an array, not ${JSON.stringify(value ?? null)}`);
  return value.map((entry, index) => of(entry, `${name}[${index}]`));
}

/**
 * The batch `applyDeltas` takes, checked here where every other value is and rebuilt member by
 * member: the motor refuses an unknown member, so a batch is normalised to the shape it reads.
 */
export function deltaBatch(raw: unknown): GraphBatch {
  if (!isRecord(raw)) throw bad("batch", "must be an object with a nodes array and an edges array");
  return { nodes: entriesOf(raw["nodes"], "nodes", nodeOf), edges: entriesOf(raw["edges"], "edges", edgeOf) };
}

export interface Deltas {
  readonly id: typeof APPLY_DELTAS;
  readonly apply: (batch: unknown) => Promise<number>;
}

/**
 * `applyDeltas`, registered and resolved here next to every other verb: the batch is read only
 * when a live session can take it. Ponytail: a sibling of `resolve`, not a dock action — an
 * `Action`'s arguments are `Args` and no control or console line carries a batch. Failing input:
 * a batch that is not `{nodes, edges}` of objects is refused here, before the worker is asked.
 * Direction: rebuilt member by member. Escape hatch: whole or nothing.
 */
export function createDeltas(reason: () => string | null, send: (batch: GraphBatch) => Promise<number>): Deltas {
  return {
    id: APPLY_DELTAS,
    apply: async (batch) => {
      const because = reason();
      if (because !== null) throw new ActionRefusal("unavailable", `\`${APPLY_DELTAS}\` cannot run: ${because}`);
      return send(deltaBatch(batch));
    },
  };
}
