import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { COLUMNS, LogRefusal, parseLog, refsOf, toRows } from "../map.mjs";
import { H, LONG, SMALL_LOG } from "./small.mjs";

const row = (rows, hash) => rows.tables[0].rows.find((r) => r.id === hash);

test("the committed small.gitlog is the fixture module's text", () => {
  assert.equal(readFileSync(new URL("small.gitlog", import.meta.url), "utf8"), SMALL_LOG);
});

test("every commit becomes one row, in log order, with its time as updatedAt", () => {
  const { rows } = toRows(parseLog(SMALL_LOG), "small");
  assert.equal(rows.source, "small");
  assert.deepEqual(rows.tables[0].columns, COLUMNS);
  assert.deepEqual(rows.tables[0].rows.map((r) => r.id), ["f", "e", "d", "c", "b", "a"].map(H));
  assert.deepEqual(rows.tables[0].rows.map((r) => r.updatedAt), [6, 5, 4, 3, 2, 1].map((s) => 1700000000 + s));
});

test("merges keep every parent in %P order and carry the merge tag", () => {
  const { rows } = toRows(parseLog(SMALL_LOG), "small");
  assert.deepEqual(row(rows, H("f")).values.parents, [H("e"), H("d"), H("c")]);
  assert.deepEqual(row(rows, H("f")).values.refs, ["v1.0", "merge"]);
  assert.deepEqual(row(rows, H("e")).values.refs, ["main", "origin/main", "merge"]);
  assert.deepEqual(row(rows, H("a")).values.refs, ["root"]);
});

test("a parent outside the log is dropped and counted", () => {
  const { rows, dropped } = toRows(parseLog(SMALL_LOG), "small");
  assert.equal(dropped, 1);
  assert.deepEqual(row(rows, H("b")).values.parents, [H("a")]);
  assert.deepEqual(row(rows, H("b")).values.refs, ["merge"], "git printed two parents");
});

test("a subject is cut at 120 code points, never inside a pair", () => {
  const [, , , c] = parseLog(SMALL_LOG);
  assert.equal(Array.from(c.subject).length, 120);
  assert.ok(c.subject.endsWith("🚀"));
  assert.equal(c.subject, LONG.slice(0, -4));
  assert.equal(parseLog(SMALL_LOG)[2].subject, "Ajouté l'accès café ☕");
});

test("an empty subject or author is an absent cell", () => {
  const log = `${[H("a"), "", "", "1", "", ""].join("\x1f")}\n`;
  const values = toRows(parseLog(log), "s").rows.tables[0].rows[0].values;
  assert.equal(values.subject, undefined);
  assert.equal(values.author, undefined);
});

test("synthetic tags never duplicate a ref", () => {
  assert.deepEqual(refsOf("merge, tag: merge", 2), ["merge"]);
  assert.deepEqual(refsOf("HEAD", 0), ["HEAD", "root"]);
  assert.deepEqual(refsOf("", 1), []);
});

test("every refusal names its line", () => {
  const good = [H("a"), "", "Ana", "1", "", "ok"];
  const cases = [
    [good.slice(0, 5), "expected 6 fields"],
    [[...good, "extra"], "expected 6 fields"],
    [[H("a").slice(0, 12), ...good.slice(1)], "hex hash"],
    [[good[0], "beef", ...good.slice(2)], "hex hash"],
    [[...good.slice(0, 3), "-1", ...good.slice(4)], "u32"],
    [[...good.slice(0, 3), "1.5", ...good.slice(4)], "u32"],
    [[...good.slice(0, 3), "4294967296", ...good.slice(4)], "u32"],
  ];
  for (const [fields, why] of cases) {
    const log = `${good.join("\x1f")}\n${fields.join("\x1f")}\n`.replace(H("a"), H("b"));
    assert.throws(() => parseLog(log), (e) => e instanceof LogRefusal && e.line === 2 && e.message.includes(why), why);
  }
  const twice = `${good.join("\x1f")}\n${good.join("\x1f")}\n`;
  assert.throws(() => parseLog(twice), (e) => e instanceof LogRefusal && e.line === 2 && /twice/.test(e.message));
});