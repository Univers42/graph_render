/**
 * What a parsed query means over one node. The grammar in `./parse.ts` says what a
 * line means; this says what it means here, and it is the only place that says it.
 *
 * The rules, in full, because every one of them is a decision the user can feel: `text`
 * is a case-insensitive substring of the label; `id`, `kind`, `db`, `path` and `group`
 * are case-insensitive equality; `tag` is case-insensitive membership, with the leading
 * `#` a user naturally types stripped; `degree` is numeric, like `version`, and both are
 * read as 0 where the document carries none. `not` negates, `and` is all, `or` is any,
 * and `all` is always true.
 */
import type { Query, QueryField } from "./parse.ts";
import type { GraphMeta } from "../source/meta.ts";

/** One node, as a query sees it: every field present, empty when the graph carries none. */
export interface QueryRow {
  readonly id: string;
  readonly label: string;
  readonly kind: string;
  readonly tags: readonly string[];
  readonly db: string;
  readonly path: string;
  readonly degree: number;
  /** The name in `meta.groups` this node's group index names; "" where the graph names none. */
  readonly group: string;
  readonly version: number;
}

/** The `field` arm of the grammar, narrowed to the one shape it can be. */
type FieldQuery = Extract<Query, { readonly kind: "field" }>;

const NO_TAGS: readonly string[] = [];

/**
 * Caveat: past `MAX_GROUPS` distinct names every further name reads as `(other groups)`, so
 * `group:` cannot tell those apart and answers the same for each of them. It errs towards
 * matching a node under a name the user did not write. The escape hatch is a `tag:` or `id:`
 * query, which reads the node itself rather than the group it was bucketed into.
 *
 * Total by construction: a node index outside the columns reads as nothing, not a throw.
 */
export function rowOf(meta: GraphMeta, node: number): QueryRow {
  return {
    id: meta.ids[node] ?? "",
    label: meta.labels[node] ?? "",
    kind: meta.kinds[node] ?? "",
    tags: meta.tags[node] ?? NO_TAGS,
    db: meta.dbs[node] ?? "",
    path: meta.paths[node] ?? "",
    degree: meta.degree[node] ?? 0,
    group: meta.groups[meta.group[node] ?? 0] ?? "",
    version: meta.versions[node] ?? 0,
  };
}

function same(given: string, wanted: string): boolean {
  return given.toLowerCase() === wanted.toLowerCase();
}

function hasTag(tags: readonly string[], wanted: string): boolean {
  const bare = wanted.startsWith("#") ? wanted.slice(1) : wanted;
  return tags.some((tag) => same(tag, bare));
}

function compares(given: number, op: string, value: string): boolean {
  const at = Number(value);
  if (op === ">") return given > at;
  if (op === "<") return given < at;
  if (op === ">=") return given >= at;
  if (op === "<=") return given <= at;
  return given === at;
}

// Ponytail: an operator that is neither `=` nor empty is read as equality. Failing
// input `kind:>record`: no ordering on strings was ever meant, and refusing here would
// make the query vanish rather than match. Direction it errs: it matches where the user
// expected nothing. Escape hatch: the parser may refuse the operator itself, and then
// the whole query is dropped instead of this one arm.
function matchesField(query: FieldQuery, row: QueryRow): boolean {
  const field: QueryField = query.field;
  if (field === "degree") return compares(row.degree, query.op, query.value);
  if (field === "version") return compares(row.version, query.op, query.value);
  if (field === "tag") return hasTag(row.tags, query.value);
  switch (field) {
    case "id": return same(row.id, query.value);
    case "kind": return same(row.kind, query.value);
    case "db": return same(row.db, query.value);
    case "path": return same(row.path, query.value);
    case "group": return same(row.group, query.value);
  }
}

export function matchesQuery(query: Query, row: QueryRow): boolean {
  if (query.kind === "all") return true;
  if (query.kind === "text") return row.label.toLowerCase().includes(query.text.toLowerCase());
  if (query.kind === "field") return matchesField(query, row);
  if (query.kind === "not") return !matchesQuery(query.of, row);
  const hits = query.of.map((one) => matchesQuery(one, row));
  return query.kind === "and" ? hits.every(Boolean) : hits.some(Boolean);
}
