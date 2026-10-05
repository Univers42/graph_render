import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { H } from "./small.mjs";

const here = new URL(".", import.meta.url).pathname;
const decorate = (field) => {
  const out = join(mkdtempSync(join(tmpdir(), "decorate-")), "out.json");
  execFileSync(process.execPath, [join(here, "../decorate.mjs"), join(here, "small.ingest.json"), join(here, "small.graph.json"), field, out]);
  return JSON.parse(readFileSync(out, "utf8"));
};

test("every commit node carries its refs as tags, and nothing else changes", () => {
  const graph = JSON.parse(readFileSync(join(here, "small.graph.json"), "utf8").split("\n")[0]);
  const doc = decorate("refs");
  assert.equal(doc.version, 1);
  assert.deepEqual(doc.edges, graph.edges);
  const byId = new Map(doc.nodes.map((n) => [n.id, n]));
  assert.deepEqual(byId.get(`small:commit:${H("e")}`).tags, ["main", "origin/main", "merge"]);
  assert.deepEqual(byId.get(`small:commit:${H("a")}`).tags, ["root"]);
  assert.deepEqual(doc.nodes.map(({ tags, ...rest }) => rest), graph.nodes);
});

test("a field that is not a list of strings is refused", () => {
  assert.throws(() => decorate("subject"), (e) => e.status === 2);
});

test("the committed ingest document is what export.mjs writes today", () => {
  const out = join(mkdtempSync(join(tmpdir(), "export-")), "small.ingest.json");
  execFileSync(process.execPath, ["--experimental-strip-types", join(here, "../export.mjs"), "small", join(here, "small.gitlog"), out], { stdio: "pipe" });
  assert.equal(readFileSync(out, "utf8"), readFileSync(join(here, "small.ingest.json"), "utf8"));
});