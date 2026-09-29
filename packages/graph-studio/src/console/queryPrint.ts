/**
 * The line that, typed in the console, reads back as the tree it was printed from. Parentheses
 * go in only where precedence needs them, so a printed query is the shortest line that still
 * says what it says — and search, filters and groups all write their tree this way.
 */
import type { Query, QueryField } from "./queryParse.ts";

const KEYWORD = /^(and|or|not)$/i;
/**
 * Everything a bare word may not hold: whitespace, a quote, a paren, a colon, a `#`, and every
 * sign that would open a comparison or an arithmetic nothing. The last group is there for the
 * field case, where a bare `>` would be read as an operator and the value would move.
 */
const RISKY = /[\s"'():#<>=$^*~+]/;

/**
 * `tag:#x` keeps its sigil bare: the `#` is the field's own punctuation, and quoting it would
 * read as a label search. A second `#` is content, and is quoted, as is `#` on its own.
 *
 * Ponytail: `id:">3"` is quoted, and `id:#x` too, where neither is a value anyone types. It
 * errs towards printing more quotes than a hand would, because the failure it avoids is a line
 * the parser reads as a different query. The escape hatch is `tag`, the one field with a sigil.
 */
function mustQuote(value: string, field: QueryField | null): boolean {
  if (value === "") return true;
  const body = field === "tag" && value.startsWith("#") ? value.slice(1) : value;
  return body === "" || KEYWORD.test(value) || RISKY.test(body);
}

function word(value: string, field: QueryField | null): string {
  if (!mustQuote(value, field)) return value;
  return `"${value.replace(/[\\"]/g, (char) => `\\${char}`)}"`;
}

/**
 * A join needs parentheses wherever precedence would read it another way, and nowhere else. A
 * `not` child is never wrapped inside a join, because `NOT` already binds tighter than both.
 *
 * Ponytail: a join of its own kind inside itself is written tight, so a hand-built
 * `and[and[a,b],c]` prints as `a AND b AND c` and reads back as `and[a,b,c]`. It errs towards
 * the shorter line, and the parser is what flattens it, so the tree comes back canonical. The
 * escape hatch is the caller: a tree the parser produced never holds one.
 */
function tight(parent: "and" | "or" | "not", child: Query): boolean {
  if (child.kind === "not") return parent === "not";
  if (child.kind !== "and" && child.kind !== "or") return false;
  if (parent === "not") return true;
  if (child.kind === "or") return true;
  return parent === "or";
}

function child(parent: "and" | "or" | "not", query: Query): string {
  const text = write(query);
  return tight(parent, query) ? `(${text})` : text;
}

/**
 * `all` is the empty query, so it is the empty line. A `not` of `all` has no such line, and
 * prints `NOT `, which the parser refuses; nothing builds that tree, and the escape hatch is
 * the parser refusing rather than this one guessing.
 */
function write(query: Query): string {
  if (query.kind === "all") return "";
  if (query.kind === "text") return word(query.text, null);
  if (query.kind === "field") return `${query.field}:${query.op}${word(query.value, query.field)}`;
  if (query.kind === "not") return `NOT ${child("not", query.of)}`;
  const joiner = query.kind === "and" ? " AND " : " OR ";
  return query.of.map((part) => child(query.kind, part)).join(joiner);
}

export function printQuery(query: Query): string {
  return write(query);
}
