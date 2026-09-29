/**
 * The one query grammar, shared by search, filters and groups so a filter is the same shape as
 * the search that filled it. `NOT` binds tighter than `AND`, `AND` tighter than `OR`, the joins
 * take any number of terms, and a group of one is that term. A query parsed here is canonical:
 * no empty join, and a join never sits inside its own join, so the printer needs no parentheses
 * it cannot justify.
 *
 * Every refusal names the 1-based column it gave up at, counting spaces, because the console
 * puts a caret under one and a count of characters is the only thing both ends can agree on.
 */
import { type QueryToken, type QueryWord, scanQuery } from "./queryLex.ts";

export type QueryField = "id" | "tag" | "kind" | "db" | "path" | "degree";

export type Query =
  | { readonly kind: "all" }
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "field"; readonly field: QueryField; readonly op: string; readonly value: string }
  | { readonly kind: "not"; readonly of: Query }
  | { readonly kind: "and"; readonly of: readonly Query[] }
  | { readonly kind: "or"; readonly of: readonly Query[] };

/** A query the grammar refuses, with the 1-based column where it gave up. */
export class QueryRefusal extends Error {
  readonly column: number;

  constructor(column: number, what: string) {
    super(`${column}: ${what}`);
    this.name = "QueryRefusal";
    this.column = column;
  }
}

const FIELDS: ReadonlyMap<string, QueryField> = new Map([
  ["id", "id"],
  ["tag", "tag"],
  ["kind", "kind"],
  ["db", "db"],
  ["path", "path"],
  ["degree", "degree"],
]);

/** Longest first, so `degree:>=3` is not read as `>` followed by `=3`. */
const OPERATORS: readonly string[] = [">=", "<=", "!=", ">", "<", "="];
const DEGREE_OPS: ReadonlySet<string> = new Set([">", "<", ">=", "<=", "="]);
const OPENS = /^[><=!$^*~+]/;
const WHOLE = /^[0-9]+$/;

interface Reader {
  readonly tokens: readonly QueryToken[];
  at: number;
}

/** What one step of a join reads: a term for `AND`, a whole `AND` chain for `OR`. */
type Term = (reader: Reader) => Query;

/** The value a field holds, with the column its first character stands at. */
interface Chunk {
  readonly text: string;
  readonly at: number;
  /** A whole quoted word: an empty one is a value, and a quote keeps any operator as content. */
  readonly quoted: boolean;
}

function peek(reader: Reader): QueryToken | null {
  return reader.tokens[reader.at] ?? null;
}

/** Just past the last token: where a query that stopped early would have carried on. */
function endColumn(reader: Reader): number {
  const last = reader.tokens[reader.tokens.length - 1];
  return last === undefined ? 1 : last.column + 1;
}

function isWord(token: QueryToken | null): token is QueryWord {
  return token !== null && token.type === "word";
}

function startsTerm(token: QueryToken | null): boolean {
  return token !== null && (token.type === "word" || token.type === "open" || token.type === "not");
}

function join(kind: "and" | "or", parts: readonly Query[]): Query {
  if (parts.length === 1) return parts[0] ?? { kind: "all" };
  return kind === "and" ? { kind: "and", of: parts } : { kind: "or", of: parts };
}

/** `(a AND b) AND c` is the join of three, not a join holding a join. */
function partsOf(query: Query, parent: "and" | "or"): readonly Query[] {
  return query.kind === parent ? query.of : [query];
}

function unknownOperator(field: QueryField, op: string): string {
  return `\`${field}:\` does not know the operator \`${op}\``;
}

function operator(field: QueryField, chunk: Chunk): string {
  if (chunk.quoted) {
    if (field === "degree") throw new QueryRefusal(chunk.at, "`degree:` needs an operator: one of > < >= <= =");
    return "";
  }
  const found = OPERATORS.find((candidate) => chunk.text.startsWith(candidate)) ?? "";
  if (found === "") {
    if (field === "degree") throw new QueryRefusal(chunk.at, "`degree:` needs an operator: one of > < >= <= =");
    if (OPENS.test(chunk.text[0] ?? "")) throw new QueryRefusal(chunk.at, unknownOperator(field, chunk.text[0] ?? ""));
    return found;
  }
  if (field === "degree" && !DEGREE_OPS.has(found)) throw new QueryRefusal(chunk.at, unknownOperator(field, found));
  return found;
}

/** A degree is a count: a non-integer is a slip worth naming, not a filter that finds nothing. */
function whole(value: string, op: string, at: number): void {
  if (!WHOLE.test(value)) throw new QueryRefusal(at, `\`degree:\` needs a whole number after \`${op}\``);
}

/** A field's value sits after the colon, and is the next word when the colon ended that one. */
function chunkOf(reader: Reader, name: string, text: string, at: number): Chunk {
  if (text !== "") return { text, at, quoted: false };
  const token = peek(reader);
  if (!isWord(token)) throw new QueryRefusal(at, `a value is missing after \`${name}:\``);
  reader.at += 1;
  return { text: token.text, at: token.column, quoted: token.quoted };
}

/**
 * `id:>` is an operator with its value still to come, and the value is the next word, so a space
 * may sit between the two. This errs towards reading `degree:> 3` as `degree:>3`, which is what
 * a hand types and what the console writes back. The escape hatch is the printer: it never
 * writes the space, so a line that came from here is always the tight one.
 */
function valueOf(reader: Reader, name: string, chunk: Chunk, op: string): string {
  const rest = chunk.text.slice(op.length);
  if (rest !== "" || chunk.quoted) return rest;
  return chunkOf(reader, name, "", chunk.at + op.length).text;
}

/**
 * A colon in a bare word always starts a field name, so `12:30` is refused as the field `12`
 * rather than searched for.
 *
 * Ponytail: it errs towards refusing a label search that happens to hold a colon, so `12:30`
 * names no node at all until it is quoted as `"12:30"`. Direction: too few results, never a
 * wrong result. The escape hatch is the quote, which is what the printer writes.
 */
function readField(reader: Reader, token: QueryWord, colon: number): Query {
  const name = token.text.slice(0, colon);
  const field = FIELDS.get(name.toLowerCase());
  if (field === undefined) throw new QueryRefusal(token.column, `\`${name.toLowerCase()}\` is not a field`);
  const chunk = chunkOf(reader, name, token.text.slice(colon + 1), token.column + colon + 1);
  const op = operator(field, chunk);
  const value = valueOf(reader, name, chunk, op);
  if (field === "degree") whole(value, op, chunk.at + op.length);
  return { kind: "field", field, op, value };
}

function readNot(reader: Reader): Query {
  const token = peek(reader);
  const column = token === null ? endColumn(reader) : token.column;
  reader.at += 1;
  const next = peek(reader);
  if (next !== null && (next.type === "and" || next.type === "or")) {
    throw new QueryRefusal(next.column, `\`${next.type.toUpperCase()}\` cannot follow \`NOT\``);
  }
  if (!startsTerm(next)) throw new QueryRefusal(column, "`NOT` has nothing to negate");
  return { kind: "not", of: readUnary(reader) };
}

function readGroup(reader: Reader): Query {
  const open = peek(reader);
  const column = open === null ? endColumn(reader) : open.column;
  reader.at += 1;
  const first = peek(reader);
  if (first !== null && first.type === "close") {
    throw new QueryRefusal(first.column, "the group between `(` and `)` is empty");
  }
  if (first === null) throw new QueryRefusal(column, "a `(` is never closed");
  const inner = readOr(reader);
  const close = peek(reader);
  if (close === null || close.type !== "close") throw new QueryRefusal(column, "a `(` is never closed");
  reader.at += 1;
  return inner;
}

/**
 * A keyword never starts a term, so a leading `AND` is a join with nothing on its left. The
 * message is written for the whole query, so a group that opens on one (`( AND a`) is told the
 * query starts with `AND` rather than that the group does.
 *
 * Ponytail: it errs towards naming the query instead of the group, so `( AND a` points at the
 * word and not at the paren that made it surprising. The escape hatch is the caret, which
 * lands on the `AND` either way.
 */
function readUnary(reader: Reader): Query {
  const token = peek(reader);
  if (token === null) throw new QueryRefusal(endColumn(reader), "the query ends where a term should start");
  if (token.type === "not") return readNot(reader);
  if (token.type === "open") return readGroup(reader);
  if (token.type === "close") throw new QueryRefusal(token.column, "a `)` has no `(`");
  if (token.type === "and" || token.type === "or") {
    throw new QueryRefusal(token.column, `the query starts with \`${token.type.toUpperCase()}\``);
  }
  if (!isWord(token)) throw new QueryRefusal(token.column, "a keyword or paren cannot start a term here");
  reader.at += 1;
  const colon = token.quoted ? -1 : token.text.indexOf(":");
  return colon > 0 ? readField(reader, token, colon) : { kind: "text", text: token.text };
}

/** `a AND b AND c` is one join of three, and the right of a keyword must be a term, not a keyword. */
function readJoin(reader: Reader, kind: "and" | "or", read: Term): Query {
  const parts: Query[] = [...partsOf(read(reader), kind)];
  for (;;) {
    const token = peek(reader);
    if (token === null || token.type !== kind) break;
    reader.at += 1;
    if (!startsTerm(peek(reader))) throw new QueryRefusal(token.column, `\`${kind.toUpperCase()}\` has nothing to join`);
    parts.push(...partsOf(read(reader), kind));
  }
  return join(kind, parts);
}

function readAnd(reader: Reader): Query {
  return readJoin(reader, "and", readUnary);
}

function readOr(reader: Reader): Query {
  return readJoin(reader, "or", readAnd);
}

function leftover(token: QueryToken): string {
  if (token.type === "close") return "a `)` has no `(`";
  if (token.type === "open") return "a `(` follows the end of the query";
  return "a word follows the end of the query";
}

export function parseQuery(text: string): Query {
  const scan = scanQuery(text);
  if (scan.failure !== null) {
    throw new QueryRefusal(scan.failure.column, `a ${scan.failure.quote} quote is never closed`);
  }
  if (scan.tokens.length === 0) return { kind: "all" };
  const reader: Reader = { tokens: scan.tokens, at: 0 };
  const query = readOr(reader);
  const left = peek(reader);
  if (left !== null) throw new QueryRefusal(left.column, leftover(left));
  return query;
}
