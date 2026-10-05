// An `Ingest` from any adapter into the ops a batch is made of: upserts, deletes, and the
// byte order the hub reads them in.
//
// Ordering is the whole point of this module. §5.3 has the hub read a workspace with
// `COLLATE "C"`, so the SDK sorts by `(collection, id)` in byte order and every document it
// writes has its keys sorted the same way. Two hubs that stored the same change then agree byte
// for byte, which is what makes an idempotency replay hash to the same text.

import type { Ingest, IngestRecord, JsonValue } from "../adapters/rows.ts";
import type { BatchWire, DeleteWire, UpsertWire } from "../hub/wire.ts";

/** `GRAPH_HUB_MAX_BATCH` of §6: 10 000 operations, and 413 past it. */
export const DEFAULT_MAX_BATCH = 10000;

/** One record's identity, which is all a delete and all a set membership test need. */
export interface SyncKey {
  readonly collection: string;
  readonly id: string;
}

/** What the SDK wants the hub to do with one record.
 *
 * `updatedAt` and `values` are the upsert's own and are absent on a delete: §5.2's
 * `DeleteWire` is an identity and nothing else, so there is no version and no cells for a store
 * to reconcile. An optional member is how one type covers both without lying about either.
 *
 * `values` is the rows adapter's `JsonValue`, not `hub.d.ts`'s: the generated one is an empty
 * interface (codegen does not inline the recursion) and so admits nothing at all, while
 * `UpsertWire.values` is `unknown` and takes this union unchanged. A source's own cell types
 * survive the wire untouched — the contract constrains the envelope, never the payload.
 */
export interface SyncOp extends SyncKey {
  readonly kind: "upsert" | "delete";
  readonly updatedAt?: number;
  readonly values?: Readonly<Record<string, JsonValue>>;
}

/** `<collection>\0<id>`: one string for both byte order and set membership.
 *
 * The NUL is below every character an id may hold, so `("ab", "c")` and `("a", "bc")` are two
 * different keys and not one. §5.1's ids may not contain a colon and the contract's ids are
 * ASCII, so a JS `<` on this string is the byte order the hub's `COLLATE "C"` scan reads.
 */
export function opKey(op: SyncKey): string {
  return `${op.collection}\0${op.id}`;
}

// What the source says should exist. A record an adapter marks `deleted: true` is *not* an op
// here: §7 and N14 make it a delete only when the hub still holds the id, and the stored set is
// known in `syncOnce`, not here. So it is simply left out of the desired set, and `deleteOps`
// turns it into a delete exactly when its id is among the stored ones — dropped otherwise, which
// is what "dropped otherwise" means. Rows that mark records deleted still keep them in
// `ingest.records`, so the decision belongs to this module and nowhere else.
export function desiredOps(ingest: Ingest): readonly SyncOp[] {
  const ops = ingest.records.filter((record) => !record.deleted).map(upsertOf);
  return ops.sort(byKey);
}

/** The stored ids that are no longer wanted, as deletes, in the same byte order.
 *
 * This is where a `deleted: true` row becomes a delete: its key is in the stored set and is not
 * in `wanted`, because `desiredOps` never names it. §5.2 refuses 422 a batch that names one id
 * in both `upserts` and `deletes`, and the caller concatenates the two lists, so `wanted` must
 * be every key `desiredOps` emitted — upserts only, which is all it emits.
 */
export function deleteOps(stored: Iterable<SyncKey>, wanted: ReadonlySet<string>): readonly SyncOp[] {
  const ops: SyncOp[] = [];
  for (const key of stored) {
    if (wanted.has(opKey(key))) continue;
    ops.push({ kind: "delete", collection: key.collection, id: key.id });
  }
  return ops.sort(byKey);
}

/** Splits `ops` into chunks of at most `maxBatch`, in order, never reordering.
 *
 * A walk, not a loop over a moving index: the ops are already in the order the hub reads, and
 * one pass is the only way a chunk cannot drop or repeat one.
 */
export function chunkOps(ops: readonly SyncOp[], maxBatch: number): readonly SyncOp[][] {
  if (!Number.isInteger(maxBatch) || maxBatch < 1) throw new RangeError(`maxBatch is a positive integer: ${maxBatch}`);
  const chunks: SyncOp[][] = [];
  for (const op of ops) {
    const last = chunks[chunks.length - 1];
    if (last === undefined || last.length >= maxBatch) chunks.push([op]);
    else last.push(op);
  }
  return chunks;
}

/** One batch: the upserts and the deletes, each already in byte order, keys sorted by bytes.
 *
 * The values map is re-sorted too, because the contract sorts the keys of every object it
 * writes, not only the top-level ones. A plain `JSON.stringify` of this is then the canonical
 * text, and that text is what the hub hashes for the idempotency key.
 */
export function batchOf(ops: readonly SyncOp[]): BatchWire {
  const upserts: UpsertWire[] = [];
  const deletes: DeleteWire[] = [];
  for (const op of ops) {
    if (op.kind === "delete") {
      deletes.push({ collection: op.collection, id: op.id });
      continue;
    }
    upserts.push({ collection: op.collection, id: op.id, updatedAt: op.updatedAt ?? 0, values: sortedValues(op.values) });
  }
  return { deletes, upserts };
}

function upsertOf(record: IngestRecord): SyncOp {
  return { kind: "upsert", collection: record.collection, id: record.id, updatedAt: record.updatedAt, values: record.values };
}

function byKey(a: SyncOp, b: SyncOp): number {
  return opKey(a) < opKey(b) ? -1 : opKey(a) > opKey(b) ? 1 : 0;
}

function sortedValues(values: Readonly<Record<string, JsonValue>> | undefined): Record<string, JsonValue> {
  const out: Record<string, JsonValue> = {};
  for (const key of Object.keys(values ?? {}).sort()) out[key] = values?.[key] as JsonValue;
  return out;
}
