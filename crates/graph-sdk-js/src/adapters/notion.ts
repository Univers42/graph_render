// The Notion adapter: Notion's property types → the ingest contract.
//
// **A pure mapping, like `rows.ts`, and that is the point.** This is where the vendor's
// twenty-odd property types are collapsed onto the contract's **eight declared roles** —
// and it is the *only* place that collapse happens, once, out loud, in a file a reader
// can check. The motor reads the roles this file writes; it never sees a `multi_select`
// and has no idea what one is. That is what makes the engine data-source-shaped rather
// than Notion-shaped, and what lets `fixtures/ingest/rows.json` and `notion.json` — the
// same dataset in two shapes — derive the same graph.
//
// ## The type-to-role table, and what it is not
//
// [`typeToRole`] is a **mapping table, not a heuristic**, and the difference is worth
// being precise about. A heuristic *infers* a role from a shape it was not told about —
// "the first `multi_select`, or a field named `/^tags?$/i`" — and a wrong inference
// **silently changes the graph**: no error, no warning, nothing in the output to detect
// it. Here a property's `type` *is* the signal, is a closed set, and every row of the
// table is a total function on it. A type the table does not name is
// [`NOT_MAPPED`][Role.Scalar], and that is the one place a value is chosen rather than
// derived — see the Ponytail below.
//
// **Ponytail (an unmapped property type).** Failing input: a property whose `type` is
// absent from [`typeToRole`] — `formula`, `created_time`, `files`, `people`, a type a
// future Notion release adds, or a typo in a caller's own export. Direction: it becomes
// `scalar`, which means **declared and read by nobody**. That is the safe direction
// (a structural role inferred wrongly would add or remove edges), but it is still
// silent: a `relation` spelled `Relation` would carry real structure and contribute
// none, and nothing in the derived graph says so. Escape hatch: the caller passes an
// explicit `roles` override, which is a declaration and therefore outranks the table —
// that is the whole point of a declared contract, and this adapter exercises it rather
// than pretending the table is complete.
//
// **No marker on the rest of the file.** The rest is a mechanical translation of nested
// values into flat cells, and a translation has nothing approximate about it.

import { RowsAdapterError } from "./rows.ts";
// `import type` for the shapes, and only that: the SDK ships no build and runs its own
// `.ts` sources through Node's type-stripping, which erases a `type`-only import but
// cannot know a named import is a type. `RowsAdapterError` is a value and comes above.
import type {
  CardinalityName,
  Ingest,
  IngestCollection,
  IngestField,
  IngestLink,
  IngestRecord,
  JsonValue,
  RoleName,
} from "./rows.ts";

/** A Notion property type, and the role it maps to. The closed set this adapter knows;
 * a type outside it becomes `scalar` (the module doc's Ponytail). */
export const typeToRole: Readonly<Record<string, RoleName>> = {
  title: "title",
  rich_text: "label",
  select: "group",
  status: "group",
  multi_select: "tags",
  relation: "link",
  number: "weight",
  // `parent` and `people` are relations, but a parent relation is a hierarchy and a
  // people field is a many-to-many membership; both are declared by the caller when
  // that is what they mean. Left unmapped here so the default stays honest.
};

/** A `relation` property, as Notion writes it. */
export interface NotionRelationConfig {
  readonly database_id: string;
  /** Absent on a plain relation, present on a `single_property` rollup-ish config. Not
   * read: a relation is a relation, and whether it is drawn with an arrowhead is a
   * declaration the caller makes through `links`, not something a config implies. */
  readonly type?: string;
}

/** One property of a Notion database: its id, its name, and its type. */
export interface NotionProperty {
  /** Notion's own property id. Stable across renames, which is why the contract's
   * field id is this and not the name. */
  readonly id: string;
  /** The name a user sees. The contract's field `name`, and the edge label a `link`
   * draws. */
  readonly name: string;
  /** The vendor type string, and the only thing this adapter reads to pick a role. */
  readonly type: string;
  readonly relation?: NotionRelationConfig;
}

/** One Notion database: its declared properties. */
export interface NotionDatabase {
  readonly id: string;
  readonly title?: { plain_text?: string } | ReadonlyArray<{ plain_text?: string }>;
  readonly properties: Readonly<Record<string, NotionProperty>>;
}

/** One Notion page: which database it is in, and its property values. */
export interface NotionPage {
  readonly id: string;
  readonly parent: { readonly database_id?: string; readonly type?: string };
  readonly properties: Readonly<Record<string, unknown>>;
  readonly deleted?: boolean;
  readonly last_edited_time?: string;
}

/** A whole Notion export, as the API's own search/list responses give it. */
export interface NotionSource {
  /** The backend these records came from; the first coordinate of every node id. The
   * API does not name its own backend, so the caller states it — a declaration, not a
   * lookup. */
  readonly source: string;
  readonly databases: readonly NotionDatabase[];
  readonly pages: readonly NotionPage[];
}

/** The caller's explicit declarations, which outrank [`typeToRole`]. Keyed by
 * `"<database id>.<property id>"`; the value is the role, or the whole field when the
 * link's target or symmetry is not derivable from the property alone. */
export interface NotionOverrides {
  /** Roles the caller declares outright. */
  readonly roles?: Readonly<Record<string, RoleName>>;
  /** Link declarations, for a property whose target collection or symmetry the caller
   * knows and the property does not say. */
  readonly links?: Readonly<Record<string, { readonly cardinality: CardinalityName; readonly symmetric?: boolean }>>;
}

function bad(path: string, what: string): never {
  throw new RowsAdapterError(path, what);
}

/** Byte order, matching the contract's writer and `rows.ts`'s column sort. `Array.sort`'s
 * default is UTF-16 code units, which disagrees with byte order outside the Basic
 * Multilingual Plane, and a property id is arbitrary text. */
function compareBytes(a: string, b: string): number {
  return Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
}

function titleOf(database: NotionDatabase): string {
  const title = database.title;
  if (title === undefined) return database.id;
  if (Array.isArray(title)) {
    return title.map((t: { plain_text?: string }) => t.plain_text ?? "").join("") || database.id;
  }
  return (title as { plain_text?: string }).plain_text ?? database.id;
}

function key(databaseId: string, propertyId: string): string {
  return `${databaseId}.${propertyId}`;
}

/** A property's link, from the property's own `relation` config, then from the
 * caller's `links` override, then declared `many` and directed. The order matters and is
 * the contract: the caller's declaration outranks the vendor's config because the
 * contract's whole premise is that a declaration outranks an inference. */
function linkOf(
  databaseId: string,
  property: NotionProperty,
  override: NotionOverrides["links"],
): IngestLink {
  const declared = override?.[key(databaseId, property.id)];
  const cardinality = declared?.cardinality ?? "many";
  const symmetric = declared?.symmetric ?? false;
  const collection = property.relation?.database_id;
  if (collection === undefined) {
    bad(
      `databases.${databaseId}.properties.${property.id}`,
      "a `link` property must name its target database, in its `relation` config or in the caller's `links`",
    );
  }
  return { collection, cardinality, symmetric };
}

function field(database: NotionDatabase, property: NotionProperty, overrides: NotionOverrides): IngestField {
  // The declaration outranks the table, which outranks nothing: an unmapped type is
  // `scalar` (the module doc's Ponytail), and an override is a declaration.
  const role = overrides.roles?.[key(database.id, property.id)] ?? typeToRole[property.type] ?? "scalar";
  const link = role === "link" ? linkOf(database.id, property, overrides.links) : null;
  return { id: property.id, name: property.name, role, link };
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
function cell(value: unknown, path: string): JsonValue {
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
function seconds(stamp: string | undefined, path: string): number {
  if (stamp === undefined) return 0;
  const millis = Date.parse(stamp);
  if (Number.isNaN(millis)) bad(path, "expected an RFC 3339 timestamp");
  return Math.floor(millis / 1000);
}

function collection(database: NotionDatabase, overrides: NotionOverrides): IngestCollection {
  const properties = Object.values(database.properties).sort((a, b) => compareBytes(a.id, b.id));
  return {
    id: database.id,
    name: titleOf(database),
    titleField: titleFieldOf(database, properties),
    // Sorted by property id, which is the contract's canonical order for a collection's
    // fields. Two exports of one database therefore produce the same document whatever
    // order the API happened to list its properties in — and so does `rows.ts`, which
    // is what makes the two adapters' documents byte-identical rather than merely equal.
    fields: properties.map((p) => field(database, p, overrides)),
  };
}

/** Which property is the title: Notion's own `title` type, or the caller's override. A
 * database with neither is refused, because a node whose label is nothing at all is a
 * node no consumer can identify — the contract's reader would refuse it too, and this
 * says so with the database to hand. */
function titleFieldOf(database: NotionDatabase, properties: readonly NotionProperty[]): string {
  const declared = properties.find((p) => p.type === "title");
  if (declared === undefined) {
    bad(`databases.${database.id}`, "no `title` property, and the contract needs one");
  }
  return declared.id;
}

function record(page: NotionPage, path: string): IngestRecord {
  const values: Record<string, JsonValue> = {};
  for (const [propertyId, value] of Object.entries(page.properties)) {
    const cellValue = cell(value, `${path}.${propertyId}`);
    // A `null` cell is an *absent* one, and the contract keeps the two apart: Notion
    // writes `null` for a property nobody has filled in, and the rows shape simply
    // leaves the key out. Writing the key with `null` would make two documents
    // describing the same data differ in bytes.
    if (cellValue !== null) values[propertyId] = cellValue;
  }
  return {
    id: page.id,
    collection: page.parent.database_id ?? bad(path, "a page must name its parent database"),
    deleted: page.deleted === true,
    updatedAt: seconds(page.last_edited_time, `${path}.last_edited_time`),
    values,
  };
}

/** Maps a Notion export to the ingest contract, or refuses with the path that says
 * where. Pure: the same export always gives the same document, and the document says
 * nothing this file decided beyond each property's declared role.
 */
export function notionToIngest(source: NotionSource, overrides: NotionOverrides = {}): Ingest {
  const collections = source.databases.map((d) => collection(d, overrides));
  const records = source.pages
    .map((p, i) => record(p, `pages[${i}]`))
    // A page in a database the export does not carry derives nothing: the contract's
    // reader would refuse the dangling collection, and saying so here names the page.
    .filter((r) => source.databases.some((d) => d.id === r.collection));
  return { version: 1, source: source.source, collections, records };
}
