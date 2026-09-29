// The convergence pair's shared half: read the two source fixtures, run the two
// adapters, and canonicalize the result the way the contract's writer does.
//
// Split out of `sdk-smoke.mjs` so the comparison is one function used by one test
// rather than a block of inline logic, and so `harness/read-snapshot-raw.mjs` — which
// reads the JSON face with no SDK at all — and this file agree about what "canonical"
// means without either importing the other.
//
// "Canonical" is the contract's own rule (`crates/graph-contract/src/ingest.rs`):
// compact, **object keys sorted by bytes at every depth**, arrays in document order.
// The Rust writer and this one are two implementations of one rule, and the test that
// matters is the one where a document written by this function is byte-identical to the
// one `graph_contract::ingest::to_json` produces for the same value — which is what
// makes "the two adapters agree" and "the Rust side agrees" the same assertion.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { notionToIngest } from "../crates/graph-sdk-js/src/adapters/notion.ts";
import { rowsToIngest } from "../crates/graph-sdk-js/src/adapters/rows.ts";

export { notionToIngest, rowsToIngest };

const HERE = dirname(fileURLToPath(import.meta.url));
const FIXTURES = join(HERE, "..", "fixtures", "ingest");

/** The caller's explicit declarations for the Notion fixture, and why each is needed.
 *
 * This is the escape hatch the adapter's Ponytail names, exercised rather than merely
 * described. A vendor property type says what Notion stored, not what it *means*: two
 * `relation` properties in the same database are a parent and a blocking dependency,
 * and the type is identical for both. So the role is declared, and the declaration
 * outranks the table.
 *
 * `body` is the same case in the other direction: a `rich_text` the reader must not
 * read, declared `scalar` so the contract says so rather than leaving the derivation to
 * wonder. `blocks` needs nothing — `relation` maps to `link`, and `many`/directed are
 * the declared defaults. */
export const NOTION_OVERRIDES = {
  roles: {
    "task.up": "parent",
    "task.body": "scalar",
    "person.reports_to": "parent",
  },
  links: {
    // `up` is a parent, so it is a one-element reference rather than a list of them.
    "task.up": { cardinality: "one", symmetric: false },
    "person.reports_to": { cardinality: "one", symmetric: false },
    "task.blocks": { cardinality: "many", symmetric: false },
  },
};

async function readFixture(name) {
  return JSON.parse(await readFile(join(FIXTURES, name), "utf8"));
}

/** The contract document `rows.ts` produces for `fixtures/ingest/rows.json`. */
export async function ingestFromRows() {
  return rowsToIngest(await readFixture("rows.json"));
}

/** The contract document `notion.ts` produces for `fixtures/ingest/notion.json`. */
export async function ingestFromNotion() {
  return notionToIngest(await readFixture("notion.json"), NOTION_OVERRIDES);
}

/** The contract document `expected-graph.json` pins: the one both adapters must match. */
export async function expectedIngest() {
  const expected = JSON.parse(await readFile(join(FIXTURES, "expected-graph.json"), "utf8"));
  return expected.ingest;
}

/** The graph `graph-core`'s derivation must produce from that document. */
export async function expectedGraph() {
  const expected = JSON.parse(await readFile(join(FIXTURES, "expected-graph.json"), "utf8"));
  return expected.graph;
}

/** Canonical JSON, by the contract's rule: compact, object keys sorted by bytes at every
 * depth, arrays untouched. `JSON.stringify` cannot do this — it preserves insertion
 * order — and the whole point of the comparison is that two adapters which *built* the
 * same object in different orders still produce the same bytes. */
export function canonicalJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  const keys = Object.keys(value).sort(compareBytes);
  return `{${keys.map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
}

/** Byte order, which is what the Rust writer sorts by and what `Array.sort`'s default
 * (UTF-16 code units) is not: they disagree on a key outside the Basic Multilingual
 * Plane. A contract document's keys are ASCII today, but the rule is stated rather than
 * inherited so it stays true if that stops being true. */
export function compareBytes(a, b) {
  return Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
}
