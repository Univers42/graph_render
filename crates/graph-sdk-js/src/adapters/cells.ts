// The cells both adapters write, and the refusals they share. Split out of `rows.ts` so
// the rules that must not drift live in one place: the finiteness predicate (B1/B2), the
// cell map's prototype and key order (M8/M9/m114/m21) and the byte comparison all three
// files were carrying copies of.
//
// Nothing here is public API except `RowsAdapterError`, which `rows.ts` re-exports so
// that every existing import path and the package's export list stay exactly as they
// were. Nothing here imports a *value* from `rows.ts` — `rows.ts` imports from here, so
// a value back the other way would be a cycle; a type-only import is erased and safe.
import type { JsonValue } from "./rows.ts";

/** The largest wire integer the contract's `u32` fields hold
 * (`docs/contract/ingest-schema.json`, `format: uint32`, D6: never `usize` on the wire).
 * Shared so the rows `updatedAt` and the Notion stamp cannot disagree about the limit. */
export const U32_MAX = 0xffffffff;

/** What an adapter refuses to map, with the path that says where. Each is a fact about
 * the *source* rather than about the graph, and each is a mistake that would otherwise
 * become a well-formed graph with nothing in it to show the mistake. */
export class RowsAdapterError extends Error {
  /** Dotted path of the offending member, e.g. `tables[0].columns[2].link`. */
  readonly path: string;
  /** What was wrong with it. */
  readonly what: string;

  // Written out rather than as TypeScript parameter properties: the SDK ships no build
  // and runs its own `.ts` sources through Node's type-stripping, which does not erase
  // parameter properties (they are not types, they are syntax it must keep).
  constructor(path: string, what: string) {
    super(`${path}: ${what}`);
    this.name = "RowsAdapterError";
    this.path = path;
    this.what = what;
  }
}

/** The one refusal, with the path that says where. `never` so a caller needs no throw. */
export function bad(path: string, what: string): never {
  throw new RowsAdapterError(path, what);
}

/** Byte order, matching the contract's writer and `harness/adapter-convergence.mjs`.
 * `Array.sort`'s default is UTF-16 code units, which disagrees with byte order outside
 * the Basic Multilingual Plane; a column or property id is arbitrary text, so the rule is
 * stated rather than inherited. */
export function compareBytes(a: string, b: string): number {
  return Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
}

/** A record's cells, as the contract document's `values` map, in one place so the two
 * adapters cannot drift on any of the four facts below.
 *
 * - **Every number is finite, at every depth.** `JSON.stringify` writes `NaN` and
 *   `Infinity` as `null`, and the derivation reads `null` as "no number" and substitutes
 *   the documented default (`crates/graph-core/src/ingest/roles.rs`, `as_number`), so a
 *   weight of `1.5` would silently become the default weight and move a layout. A cell is
 *   refused with the path of the offending number instead — nested too, because a `NaN`
 *   inside an array or an object cell serialises to `null` just the same.
 * - **`__proto__` is an ordinary key.** The map is built with `Object.create(null)`, so a
 *   cell keyed `__proto__` becomes an own key instead of setting the prototype and
 *   vanishing while its field stays declared in the document. The name is not refused:
 *   it is a legal source column name, and the vanishing is the bug.
 * - **`undefined` is an absent cell, not a present empty one.** The key is skipped, which
 *   is what the Notion side already did for a `null` cell, and it is what makes the two
 *   adapters produce identical members for identical data: a key that
 *   `JSON.stringify` omits would be a member of the object and absent from the wire.
 * - **Keys are emitted in byte order**, the contract's canonical order for object keys.
 *
 * One honest limit: JavaScript re-orders *integer-like* own keys numerically whatever
 * order they are inserted in, so for a column named `10` this order is best effort. The
 * canonical byte order of a document is the canonical writer's job — `canonicalJson` in
 * `harness/adapter-convergence.mjs`, `graph_contract::ingest::to_json` on the Rust side —
 * and that is the rule the contract states. For every other key, which is every key a
 * real source uses, the order here is exact. */
export function cellValues(
  cells: ReadonlyArray<readonly [string, JsonValue]>,
  path: string,
): Record<string, JsonValue> {
  const values = Object.create(null) as Record<string, JsonValue>;
  const sorted = [...cells].sort((a, b) => compareBytes(a[0], b[0]));
  for (const [key, value] of sorted) {
    if (value === undefined) continue;
    finiteAtDepth(value, `${path}.${key}`);
    values[key] = value;
  }
  return values;
}

/** Refuse a non-finite number anywhere in a cell, naming where it is. Depth, not just the
 * top level: `JSON.stringify` nulls a nested `NaN` exactly as it nulls a top-level one,
 * and the derivation cannot tell the two apart from the document. */
function finiteAtDepth(value: JsonValue, path: string): void {
  if (typeof value === "number" && !Number.isFinite(value)) {
    bad(path, "a number must be finite: JSON writes NaN and Infinity as null");
  }
  if (Array.isArray(value)) {
    value.forEach((item, i) => finiteAtDepth(item, `${path}[${i}]`));
    return;
  }
  if (typeof value === "object" && value !== null) {
    for (const [key, item] of Object.entries(value)) finiteAtDepth(item, `${path}.${key}`);
  }
}

/** A wire `u32`, or the refusal that says where. An absent version is the caller's
 * business (`0`), so this reads only a value that is present. */
export function u32(value: number, path: string): number {
  if (!Number.isInteger(value) || value < 0 || value > U32_MAX) {
    bad(path, `expected a u32: an integer in 0..${U32_MAX}`);
  }
  return value;
}