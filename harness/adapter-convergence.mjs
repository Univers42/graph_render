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

import { readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { notionToIngest } from "../crates/graph-sdk-js/src/adapters/notion.ts";
import { RowsAdapterError, rowsToIngest } from "../crates/graph-sdk-js/src/adapters/rows.ts";
import { check, fail } from "./sdk-smoke/lib.mjs";

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

/** The contract document `expected-graph.json` pins: the one both adapters must match.
 *
 * Guarded, because a *committed fixture* missing a member is "could not run", not a
 * failure to report: unguarded, the reader handed `undefined` to `canonicalJson`, and the
 * comparison died in `Math.min(len, undefined)` — an uncaught `TypeError`, exit 1, and
 * the "first difference at byte N" report lost. `fail` is the harness's own exit-2. */
export async function expectedIngest() {
  return requiredMember(await readExpected(), "ingest");
}

/** The graph `graph-core`'s derivation must produce from that document. Guarded for the
 * same reason, and for the same reason it is a *different* member: a fixture can pin the
 * graph and not the document. */
export async function expectedGraph() {
  return requiredMember(await readExpected(), "graph");
}

/** Exported so a test can hand it a document that *is* missing the member: the committed
 * fixture has both, so the guard cannot be observed by reading it. */
export function requiredMember(expected, name) {
  if (expected[name] === undefined) {
    fail(`expected-graph.json has no \`${name}\` member, so there is nothing to compare against`);
  }
  return expected[name];
}

function readExpected() {
  return JSON.parse(readFileSync(join(FIXTURES, "expected-graph.json"), "utf8"));
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

// ---------------------------------------------------------------- negative cases
//
// **At module scope on purpose.** This module is imported by `harness/sdk-smoke/
// convergence.mjs`, which the `sdk-adapter-convergence` gate row runs, and that row is the
// only place these can execute: a TypeScript unit test is not a gate row, and a refusal
// nothing runs is not a control. So the cases are computed once at import time, reported
// through `check` (so they land in the same "ok/not ok + # pass" summary the run already
// prints), *and* exported — so a test can assert the same four facts without a second
// implementation of them. Synchronous throughout: `check` takes a boolean, not a promise.

/** The sources these cases run on, built inline rather than read from a fixture: the
 * fixtures hold no non-finite cell, no `__proto__` column and no orphan page, which is
 * exactly why the fixture cannot be the control here. */
const NAN_ROWS = {
  source: "s",
  tables: [
    {
      id: "t",
      titleColumn: "name",
      columns: [
        { name: "name", role: "title" },
        { name: "effort", role: "weight" },
      ],
      rows: [{ id: "r", values: { name: "n", effort: Number.NaN } }],
    },
  ],
};

const NESTED_INFINITY_ROWS = {
  source: "s",
  tables: [
    {
      id: "t",
      titleColumn: "name",
      columns: [
        { name: "name", role: "title" },
        { name: "effort", role: "weight" },
      ],
      rows: [{ id: "r", values: { name: "n", effort: [1, Number.POSITIVE_INFINITY] } }],
    },
  ],
};

const NAN_NOTION = {
  source: "s",
  databases: [
    {
      id: "d",
      properties: {
        name: { id: "name", name: "Name", type: "title" },
        effort: { id: "effort", name: "Effort", type: "number" },
      },
    },
  ],
  pages: [
    {
      id: "p",
      parent: { database_id: "d" },
      properties: { name: { title: [{ plain_text: "n" }] }, effort: { number: Number.NaN } },
    },
  ],
};

/** A map with `__proto__` as an **own** key, which an object literal cannot write:
 * `__proto__:` in a literal sets the prototype and the key vanishes. Both adapters are
 * asked the same question a source with that column or property name would ask. */
function withProto(properties, proto) {
  const map = Object.defineProperty({}, "__proto__", {
    value: proto,
    enumerable: true,
    writable: true,
    configurable: true,
  });
  return Object.assign(map, properties);
}

const PROTO_ROWS = {
  source: "s",
  tables: [
    {
      id: "t",
      titleColumn: "name",
      columns: [
        { name: "name", role: "title" },
        { name: "__proto__", role: "scalar" },
      ],
      rows: [{ id: "r", values: withProto({ name: "n" }, "boom") }],
    },
  ],
};

const PROTO_NOTION = {
  source: "s",
  databases: [
    {
      id: "d",
      properties: withProto(
        { name: { id: "name", name: "Name", type: "title" } },
        { id: "__proto__", name: "Proto", type: "number" },
      ),
    },
  ],
  pages: [
    {
      id: "p",
      parent: { database_id: "d" },
      properties: withProto({ name: { title: [{ plain_text: "n" }] } }, { number: 1 }),
    },
  ],
};

/** Whether `run` refuses with a `RowsAdapterError` whose `path` names the offending cell.
 * The path is half the assertion: a refusal that cannot say where is the failure mode
 * these four cases exist to catch in the other direction. */
function refusesCell(run, pattern) {
  try {
    run();
    return false;
  } catch (error) {
    return error instanceof RowsAdapterError && pattern.test(error.path);
  }
}

export const adapterRefusals = [
  {
    name: "a NaN weight cell is refused, naming the cell (B1)",
    ok: refusesCell(() => rowsToIngest(NAN_ROWS), /^tables\[0\]\.rows\[0\]\.values\.effort$/),
  },
  {
    name: "an Infinity nested inside an array cell is refused, naming the position (B1)",
    ok: refusesCell(() => rowsToIngest(NESTED_INFINITY_ROWS), /^tables\[0\]\.rows\[0\]\.values\.effort\[1\]$/),
  },
  {
    name: "a NaN number property is refused, naming the property (B2)",
    ok: refusesCell(() => notionToIngest(NAN_NOTION), /^pages\[0\]\.effort\.number$/),
  },
  {
    name: "a `__proto__` cell survives as an own key, through both adapters (M8/M9)",
    ok:
      Object.hasOwn(rowsToIngest(PROTO_ROWS).records[0].values, "__proto__") &&
      Object.hasOwn(notionToIngest(PROTO_NOTION).records[0].values, "__proto__"),
  },
];

for (const refusal of adapterRefusals) check(refusal.name, refusal.ok);

// ---------------------------------------------------------------- m31: every override key is consumed

/** Every `"<database id>.<property id>"` pair the Notion fixture's databases declare.
 * Derived from the fixture rather than hand-written, so it stays true as the fixture
 * changes. */
function declaredOverrideKeys() {
  const notion = JSON.parse(readFileSync(join(FIXTURES, "notion.json"), "utf8"));
  const keys = [];
  for (const database of notion.databases) {
    for (const property of Object.values(database.properties)) keys.push(`${database.id}.${property.id}`);
  }
  return new Set(keys);
}

const DECLARED_KEYS = declaredOverrideKeys();
for (const section of ["roles", "links"]) {
  for (const declared of Object.keys(NOTION_OVERRIDES[section] ?? {})) {
    check(
      `every override key is one the fixture declares: ${declared}`,
      DECLARED_KEYS.has(declared),
    );
  }
}
