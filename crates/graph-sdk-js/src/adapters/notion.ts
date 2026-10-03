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

// The shared cell map, the byte comparison and the one refusal: `cells.ts`, which the
// rows adapter reads too. That shared module is what makes B1/B2 one rule — a `NaN` cell
// is refused from whichever side it arrives, by the same predicate.
import { bad, cellValues, compareBytes } from "./cells.ts";
import { cell, seconds } from "./notion-cells.ts";
// `import type` for the shapes, and only that: the SDK ships no build and runs its own
// `.ts` sources through Node's type-stripping, which erases a `type`-only import but
// cannot know a named import is a type. `bad` is a value and comes above.
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

function titleOf(database: NotionDatabase): string {
  const title = database.title;
  if (title === undefined) return database.id;
  if (Array.isArray(title)) {
    return title.map((t: { plain_text?: string }) => t.plain_text ?? "").join("") || database.id;
  }
  return (title as { plain_text?: string }).plain_text ?? database.id;
}

/** The override key: `"<database id>.<property id>"`. The dot is not escaped, which is a
 * real limitation of the format — databases `a.b` (property `c`) and `a` (property
 * `b.c`) both read `a.b.c`. The format is public surface (`NotionOverrides` is exported
 * and `harness/adapter-convergence.mjs`'s `NOTION_OVERRIDES` is written in it), so it
 * cannot change; [`assertDistinctKeys`] makes the collision loud instead. */
function key(databaseId: string, propertyId: string): string {
  return `${databaseId}.${propertyId}`;
}

/** Refuse two distinct property pairs that the dotted key format reads identically, naming
 * both. Silently, one declaration landed on the other's property and the caller had no
 * way to see it: nothing else in the mapping would notice.
 *
 * Only *addressed* pairs are collected — a collision nobody declares loses nothing, and
 * refusing on it would be refusing a legal export. */
function assertDistinctKeys(databases: readonly NotionDatabase[], overrides: NotionOverrides): void {
  const addressed = new Set([...Object.keys(overrides.roles ?? {}), ...Object.keys(overrides.links ?? {})]);
  const seen = new Map<string, string>();
  for (const database of databases) {
    for (const property of Object.values(database.properties)) {
      const pair = key(database.id, property.id);
      if (!addressed.has(pair)) continue;
      const path = `databases.${database.id}.properties.${property.id}`;
      const other = seen.get(pair);
      if (other !== undefined) {
        bad(`${other} and ${path}`, `both read the override key \`${pair}\`, which does not escape the dot`);
      }
      seen.set(pair, path);
    }
  }
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

function collection(database: NotionDatabase, overrides: NotionOverrides): IngestCollection {
  const properties = Object.values(database.properties).sort((a, b) => compareBytes(a.id, b.id));
  return {
    id: database.id,
    name: titleOf(database),
    titleField: titleFieldOf(database, properties, overrides),
    // Sorted by property id, which is the contract's canonical order for a collection's
    // fields. Two exports of one database therefore produce the same document whatever
    // order the API happened to list its properties in — and so does `rows.ts`, which
    // is what makes the two adapters' documents byte-identical rather than merely equal.
    fields: properties.map((p) => field(database, p, overrides)),
  };
}

/** Which property is the title. The caller's override first, then Notion's own `title`
 * type — the file's own rule, a declaration outranks the table, applied here as well as in
 * `field`. Reading only the `title` type got both directions wrong: a database whose
 * title is declared by override was refused, and an override that *moved* the
 * title-typed property produced a document naming a field the reader then refused.
 *
 * A database with neither is refused, because a node whose label is nothing at all is a
 * node no consumer can identify, and this says so with the database to hand. */
function titleFieldOf(
  database: NotionDatabase,
  properties: readonly NotionProperty[],
  overrides: NotionOverrides,
): string {
  const declared = overrides.roles
    ? properties.filter((p) => overrides.roles?.[key(database.id, p.id)] === "title")
    : [];
  const chosen = declared[0] ?? properties.find((p) => p.type === "title");
  if (chosen === undefined) {
    bad(`databases.${database.id}`, "no `title` property, and the contract needs one");
  }
  return chosen.id;
}

function record(page: NotionPage, path: string, databaseIds: ReadonlySet<string>): IngestRecord {
  const collection = page.parent.database_id ?? bad(path, "a page must name its parent database");
  // A page whose parent the export does not carry derives nothing, and saying so is the
  // whole point: the contract's reader would refuse the dangling collection. Dropping the
  // page was the only silent record loss in either adapter, with no error, no count and
  // no path.
  if (!databaseIds.has(collection)) {
    bad(path, `its parent database \`${collection}\` is absent from the export's \`databases\``);
  }
  // A `null` cell is an *absent* one, and the contract keeps the two apart: Notion writes
  // `null` for a property nobody has filled in, and the rows shape simply leaves the key
  // out. Writing the key with `null` would make two documents describing the same data
  // differ in bytes. The surviving cells go through `cells.ts`, which is also where the
  // `__proto__` own-key and byte-order rules live.
  const cells: [string, JsonValue][] = [];
  for (const [propertyId, value] of Object.entries(page.properties)) {
    const cellValue = cell(value, `${path}.${propertyId}`);
    if (cellValue !== null) cells.push([propertyId, cellValue]);
  }
  return {
    id: page.id,
    collection,
    deleted: page.deleted === true,
    updatedAt: seconds(page.last_edited_time, `${path}.last_edited_time`),
    values: cellValues(cells, path),
  };
}

/** Maps a Notion export to the ingest contract, or refuses with the path that says
 * where. Pure: the same export always gives the same document, and the document says
 * nothing this file decided beyond each property's declared role.
 */
export function notionToIngest(source: NotionSource, overrides: NotionOverrides = {}): Ingest {
  assertDistinctKeys(source.databases, overrides);
  const databaseIds = new Set(source.databases.map((d) => d.id));
  const collections = source.databases.map((d) => collection(d, overrides));
  const records = source.pages.map((p, i) => record(p, `pages[${i}]`, databaseIds));
  return { version: 1, source: source.source, collections, records };
}
