// The ingest adapters' refusals and the cells they write (B1, B2, M8, M9, M10, m21,
// m114, m23–m28 from `docs/reviews/review-harness-sdk.md`).
//
// Every source here is inline and tiny: an adapter that only refuses the committed
// fixture's shapes is not refusing anything, and these cases are shapes the fixture
// does not contain. The only things read from disk are `notion.json` and
// `expected-graph.json`, and they are read by the module under test — never by an
// expectation written here.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/adapters.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

import * as barrel from "../src/adapters.ts";
import { notionToIngest, typeToRole } from "../src/adapters/notion.ts";
import { RowsAdapterError, rowsToIngest } from "../src/adapters/rows.ts";

/** An object with `key` as an **own** property. An object literal cannot say one:
 * `__proto__:` in a literal sets the prototype, which is exactly the trap M8 and M9 are
 * about — so the source under test is built the way a parsed JSON document builds it. */
function ownKey(key, value) {
  return Object.defineProperty({}, key, { value, enumerable: true, writable: true, configurable: true });
}

/** A rows source of one table: `columns` as declared, `rows` as given. */
function rowsSource(columns, rows) {
  return {
    source: "s",
    tables: [{ id: "t", titleColumn: "name", columns, rows }],
  };
}

/** The two-column table most cases below use, plus its row. */
function weightSource(values) {
  return rowsSource(
    [
      { name: "name", role: "title" },
      { name: "effort", role: "weight" },
    ],
    [{ id: "r", values }],
  );
}

/** The three properties a Notion database declares below: a `title`, a `number` and a
 * `rich_text` — one per interesting mapping. */
const PROPERTIES = {
  name: { id: "name", name: "Name", type: "title" },
  effort: { id: "effort", name: "Effort", type: "number" },
  head: { id: "head", name: "Head", type: "rich_text" },
};

const TITLE_CELL = { name: { title: [{ plain_text: "n" }] } };

/** A Notion source: one database `d` declaring `properties`, and `pages`. */
function notionSource(properties, pages) {
  return {
    source: "s",
    databases: [{ id: "d", properties }],
    pages: pages ?? [{ id: "p", parent: { database_id: "d" }, properties: TITLE_CELL }],
  };
}

test("B1: rowsToIngest refuses a non-finite cell naming its path", () => {
  assert.throws(
    () => rowsToIngest(weightSource({ name: "n", effort: Number.NaN })),
    (error) => {
      assert.ok(error instanceof RowsAdapterError, "a RowsAdapterError");
      assert.equal(error.path, "tables[0].rows[0].values.effort");
      return true;
    },
  );
  assert.throws(
    () => rowsToIngest(weightSource({ name: "n", effort: Number.POSITIVE_INFINITY })),
    RowsAdapterError,
  );
});

test("B1: rowsToIngest refuses a non-finite number nested inside an array or object cell", () => {
  assert.throws(
    () => rowsToIngest(weightSource({ name: "n", effort: [1, 2, Number.NaN] })),
    (error) => {
      assert.ok(error instanceof RowsAdapterError);
      assert.equal(error.path, "tables[0].rows[0].values.effort[2]");
      return true;
    },
  );
  assert.throws(
    () => rowsToIngest(weightSource({ name: "n", effort: { a: { b: Number.NEGATIVE_INFINITY } } })),
    (error) => {
      assert.equal(error.path, "tables[0].rows[0].values.effort.a.b");
      return true;
    },
  );
});

test("B2: notionToIngest refuses a non-finite number property naming its path", () => {
  const source = notionSource(PROPERTIES, [
    { id: "p", parent: { database_id: "d" }, properties: { ...TITLE_CELL, effort: { number: Number.NaN } } },
  ]);
  assert.throws(
    () => notionToIngest(source),
    (error) => {
      assert.ok(error instanceof RowsAdapterError, "a RowsAdapterError");
      assert.equal(error.path, "pages[0].effort.number");
      return true;
    },
  );
});

test("M8: a rows cell keyed `__proto__` survives as an own key", () => {
  const values = ownKey("__proto__", "boom");
  Object.assign(values, { name: "n", effort: 1 });
  const source = weightSource(values);
  source.tables[0].columns.push({ name: "__proto__", role: "scalar" });
  const record = rowsToIngest(source).records[0];
  assert.ok(Object.hasOwn(record.values, "__proto__"), "an own key, not a prototype");
  assert.equal(record.values["__proto__"], "boom");
  assert.ok(JSON.stringify(record.values).includes('"__proto__":"boom"'), "and it reaches the wire text");
});

test("M9: a notion property keyed `__proto__` survives as an own key", () => {
  const properties = ownKey("__proto__", { number: 1 });
  Object.assign(properties, TITLE_CELL);
  const record = notionToIngest(notionSource(PROPERTIES, [
    { id: "p", parent: { database_id: "d" }, properties },
  ])).records[0];
  assert.ok(Object.hasOwn(record.values, "__proto__"), "an own key, not a prototype");
  assert.equal(record.values["__proto__"], 1);
  assert.ok(JSON.stringify(record.values).includes('"__proto__":1'), "and it reaches the wire text");
});

test("M10: a declared `link.symmetric` that is not a boolean is refused", () => {
  const column = { name: "at", role: "link", link: { collection: "u", cardinality: "many", symmetric: "yes" } };
  const source = rowsSource([{ name: "name", role: "title" }, column], [{ id: "r", values: { name: "n" } }]);
  assert.throws(
    () => rowsToIngest(source),
    (error) => {
      assert.ok(error instanceof RowsAdapterError);
      assert.equal(error.path, "tables[0].columns[1]");
      return true;
    },
  );
  // A boolean survives as itself, and an absent one is the documented `false`.
  const field = (document) => document.collections[0].fields.find((f) => f.id === "at");
  column.link.symmetric = true;
  assert.equal(field(rowsToIngest(source)).link.symmetric, true);
  delete column.link.symmetric;
  assert.equal(field(rowsToIngest(source)).link.symmetric, false);
});

test("m21: an `undefined` cell is absent, exactly as a Notion `null` cell already was", () => {
  const record = rowsToIngest(weightSource({ name: "n", effort: undefined })).records[0];
  assert.deepEqual(Object.keys(record.values), ["name"], "no own key for an undefined cell");
  assert.equal(JSON.parse(JSON.stringify(record.values)).effort, undefined, "and none on the wire");
  const notion = notionToIngest(notionSource(PROPERTIES, [
    { id: "p", parent: { database_id: "d" }, properties: { ...TITLE_CELL, effort: { number: null } } },
  ])).records[0];
  assert.deepEqual(Object.keys(notion.values), ["name"], "a null cell was already absent");
  assert.deepEqual(Object.keys(record.values), Object.keys(notion.values), "so the two documents agree");
});

test("m114: cells are emitted in byte order of the key, in both adapters", () => {
  const source = rowsSource(
    [
      { name: "z", role: "scalar" },
      { name: "a", role: "scalar" },
      { name: "name", role: "title" },
    ],
    [{ id: "r", values: { z: "zz", a: "aa", name: "n" } }],
  );
  assert.deepEqual(Object.keys(rowsToIngest(source).records[0].values), ["a", "name", "z"]);
  const notion = notionToIngest(notionSource(
    {
      ...PROPERTIES,
      zulu: { id: "zulu", name: "Zulu", type: "rich_text" },
      alpha: { id: "alpha", name: "Alpha", type: "rich_text" },
    },
    [
      {
        id: "p",
        parent: { database_id: "d" },
        properties: {
          zulu: { rich_text: [{ plain_text: "z" }] },
          alpha: { rich_text: [{ plain_text: "a" }] },
          ...TITLE_CELL,
        },
      },
    ],
  )).records[0];
  assert.deepEqual(Object.keys(notion.values), ["alpha", "name", "zulu"]);
});

test("m23: a rows `updatedAt` outside the contract's `u32` is refused; an absent one is 0", () => {
  for (const value of [-1, 1.5, 2 ** 32, Number.NaN]) {
    const source = weightSource({ name: "n" });
    source.tables[0].rows[0].updatedAt = value;
    assert.throws(
      () => rowsToIngest(source),
      (error) => {
        assert.ok(error instanceof RowsAdapterError, `a RowsAdapterError for ${value}`);
        assert.equal(error.path, "tables[0].rows[0].updatedAt");
        return true;
      },
    );
  }
  assert.equal(rowsToIngest(weightSource({ name: "n" })).records[0].updatedAt, 0);
  const top = weightSource({ name: "n" });
  top.tables[0].rows[0].updatedAt = 4294967295;
  assert.equal(rowsToIngest(top).records[0].updatedAt, 4294967295, "u32::MAX is in range");
});

test("m24: a declared title outranks the `title` type, in both directions", () => {
  const overrides = { roles: { "d.head": "title" } };
  // No `title`-typed property at all, the title declared: previously refused.
  const declared = notionSource({ head: PROPERTIES.head }, [
    { id: "p", parent: { database_id: "d" }, properties: { head: { rich_text: [{ plain_text: "h" }] } } },
  ]);
  const collection = notionToIngest(declared, overrides).collections[0];
  assert.equal(collection.titleField, "head");
  assert.equal(collection.fields.find((f) => f.id === "head").role, "title");
  // A `title`-typed property the override moves: the declaration wins, so the document
  // names the declared field instead of one the reader would refuse.
  const moved = notionSource({ ...PROPERTIES }, [
    { id: "p", parent: { database_id: "d" }, properties: TITLE_CELL },
  ]);
  assert.equal(notionToIngest(moved, overrides).collections[0].titleField, "head");
  // With neither a `title` type nor a declaration, the database is still refused, by path.
  assert.throws(
    () => notionToIngest(declared),
    (error) => {
      assert.ok(error instanceof RowsAdapterError);
      assert.equal(error.path, "databases.d");
      return true;
    },
  );
});

test("m25: two property pairs that read one override key are refused, naming both", () => {
  const source = notionSource(PROPERTIES);
  const title = { id: "t", name: "T", type: "title" };
  source.databases.push(
    { id: "a.b", properties: { t: title, c: { id: "c", name: "C", type: "rich_text" } } },
    { id: "a", properties: { t: title, "b.c": { id: "b.c", name: "BC", type: "rich_text" } } },
  );
  assert.throws(
    () => notionToIngest(source, { roles: { "a.b.c": "scalar" } }),
    (error) => {
      assert.ok(error instanceof RowsAdapterError, "a RowsAdapterError");
      assert.match(error.path, /databases\.a\.b\.properties\.c/, "the first pair");
      assert.match(error.path, /databases\.a\.properties\.b\.c/, "and the second");
      return true;
    },
  );
  // Nothing is declared on that key, so nothing can land on the wrong property.
  assert.doesNotThrow(() => notionToIngest(source));
});

test("m26: a page whose parent database the export does not carry is refused", () => {
  const source = notionSource(PROPERTIES, [
    { id: "p", parent: { database_id: "d" }, properties: TITLE_CELL },
    { id: "orphan", parent: { database_id: "missing" }, properties: TITLE_CELL },
  ]);
  assert.throws(
    () => notionToIngest(source),
    (error) => {
      assert.ok(error instanceof RowsAdapterError);
      assert.equal(error.path, "pages[1]");
      return true;
    },
  );
});

test("m27: a stamp outside the contract's `u32` seconds is refused", () => {
  const page = (stamp) => ({ id: "p", parent: { database_id: "d" }, properties: TITLE_CELL, last_edited_time: stamp });
  const refused = (stamp) =>
    assert.throws(
      () => notionToIngest(notionSource(PROPERTIES, [page(stamp)])),
      (error) => {
        assert.ok(error instanceof RowsAdapterError);
        assert.equal(error.path, "pages[0].last_edited_time");
        return true;
      },
    );
  refused("1969-12-31T23:59:59.500Z");
  refused("+02070-01-01T00:00:00.000Z");
  // In range: 2106-02-07T06:28:15Z is the last second a `u32` seconds holds.
  const top = notionToIngest(notionSource(PROPERTIES, [page("2106-02-07T06:28:15.000Z")]));
  assert.equal(top.records[0].updatedAt, 4294967295);
  assert.equal(notionToIngest(notionSource(PROPERTIES, [page(undefined)])).records[0].updatedAt, 0);
});

test("M28: the barrel re-exports `typeToRole`, the table the escape hatch is defined against", () => {
  assert.equal(barrel.typeToRole, typeToRole);
  assert.equal(barrel.typeToRole.number, "weight");
});
