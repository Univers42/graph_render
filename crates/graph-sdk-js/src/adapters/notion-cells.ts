// The flat-cell translation of Notion property values, split out of `notion.ts`
// to keep each file under the line limit. Nothing here is public API.

import { RowsAdapterError } from "./rows.ts";
import type { JsonValue } from "./rows.ts";

export function bad(path: string, what: string): never {
  throw new RowsAdapterError(path, what);
}

/** One property value, as flat contract cells.
 *
 * Notion wraps every value in a type-tagged object (`{ title: [...] }`,
 * `{ select: { name } }`), so unwrapping is the mechanical part.
 *
 * **An empty container is an absent cell, not an empty one.** Notion spells "unset" as
 * an empty array or `null` — a page whose relation is empty writes
 * `{ "relation": [] }` — and the contract distinguishes a missing cell from a present
 * empty one, so this maps the first to the second. A shape this does not recognise
 * yields `null` for the same reason: a cell the derivation cannot read is absent, and
 * absent is a fact the contract can state. Guessing here would put a plausible string
 * in a `title` field, which is the direction that matters. */
export function cell(value: unknown, path: string): JsonValue {
  if (value === null || typeof value === "boolean" || typeof value === "number" || typeof value === "string") {
    return value;
  }
  if (Array.isArray(value)) {
    const items = value.map((v, i) => cell(v, `${path}[${i}]`));
    return items.length === 0 ? null : items;
  }
  if (typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length !== 1) return null;
    const entry = entries[0] as [string, unknown];
    const key = entry[0];
    const inner = entry[1];
    switch (key) {
      case "title":
      case "rich_text":
        return plain(inner, `${path}.${key}`);
      case "select":
      case "status":
        return name(inner, `${path}.${key}`);
      case "multi_select":
        return names(inner, `${path}.${key}`);
      case "relation":
        return relations(inner, `${path}.${key}`);
      case "number":
        return typeof inner === "number" ? inner : null;
      default:
        return null;
    }
  }
  return null;
}

/** Notion's rich text: an array of `{ plain_text }` fragments, concatenated. An empty
 * array is `null`, because Notion writes it for a property nobody has filled in. */
function plain(value: unknown, path: string): string | null {
  if (!Array.isArray(value)) bad(path, "expected an array of rich text");
  const text = value.map((f) => (f as { plain_text?: string }).plain_text ?? "").join("");
  return text === "" ? null : text;
}

/** Notion's `select`/`status`: `{ name }`, or `null` when unset. */
function name(value: unknown, path: string): string | null {
  if (value === null) return null;
  const text = (value as { name?: unknown }).name;
  if (typeof text !== "string") bad(path, "expected a named option");
  return text;
}

/** Notion's `multi_select`: `[{ name }]`, in the order Notion listed them. An empty
 * array is `null` — the same "unset" reading as everywhere else here. */
function names(value: unknown, path: string): string[] | null {
  if (!Array.isArray(value)) bad(path, "expected an array of options");
  if (value.length === 0) return null;
  return value.map((option, i) => {
    const text = (option as { name?: unknown }).name;
    if (typeof text !== "string") bad(`${path}[${i}]`, "expected a named option");
    return text;
  });
}

/** Notion's `relation`: `[{ id }]`, in the order Notion listed them. An empty array is
 * `null`; a one-element array is left as a list, because the `parent` role reads
 * exactly that (see `graph_core::ingest::roles::parent`). */
function relations(value: unknown, path: string): string[] | null {
  if (!Array.isArray(value)) bad(path, "expected an array of relations");
  if (value.length === 0) return null;
  return value.map((relation, i) => {
    const id = (relation as { id?: unknown }).id;
    if (typeof id !== "string") bad(`${path}[${i}]`, "expected a relation id");
    return id;
  });
}

/** A page's `last_edited_time` (RFC 3339) as the contract's `u32` seconds. Notion
 * stamps milliseconds, so the division is exact and the value stays inside `u32` until
 * 2106 — which is a real limit and a documented one, not a silent truncation. A missing
 * or unparseable stamp is `0`, the same default a source with no version column gets. */
export function seconds(stamp: string | undefined, path: string): number {
  if (stamp === undefined) return 0;
  const millis = Date.parse(stamp);
  if (Number.isNaN(millis)) bad(path, "expected an RFC 3339 timestamp");
  return Math.floor(millis / 1000);
}
