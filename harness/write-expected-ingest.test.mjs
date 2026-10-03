// What `write-expected-ingest.mjs` must not do: touch the `graph` half. Three runtimes
// pin that half by `include_str!`/`readFile`, so a regeneration of the `ingest` half
// that quietly nulls it is silent, total data loss behind a zero exit code.
//
// Every case writes to a temp copy; the committed fixture is never a target here. The
// gate rows that *compare* are `sdk-adapter-convergence` and `graph-core ingest`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const run = promisify(execFile);
const HERE = dirname(fileURLToPath(import.meta.url));
const WRITER = join(HERE, "write-expected-ingest.mjs");
const FIXTURE = join(HERE, "..", "fixtures", "ingest", "expected-graph.json");
const WORK = await mkdtemp(join(tmpdir(), "write-expected-ingest-"));

/** The committed fixture's `graph` half: the derivation three runtimes pin. */
const { graph } = JSON.parse(await readFile(FIXTURE, "utf8"));

/** The writer, as `{ code, stdout, stderr }`: the exit code is the assertion. */
async function writer(out) {
  try {
    const { stdout, stderr } = await run(
      process.execPath,
      ["--experimental-strip-types", WRITER, "--out", out],
      { cwd: join(HERE, ".."), maxBuffer: 64 * 1024 * 1024 },
    );
    return { code: 0, stdout, stderr };
  } catch (err) {
    return { code: err.code, stdout: err.stdout ?? "", stderr: err.stderr ?? "" };
  }
}

const copy = async (name, value) => {
  const path = join(WORK, name);
  await writeFile(path, typeof value === "string" ? value : JSON.stringify(value, null, 2));
  return path;
};

test("an existing graph half survives byte for byte (M13)", async () => {
  const path = await copy("graph-only.json", { graph });
  const before = await readFile(path, "utf8");
  const { code, stderr } = await writer(path);
  assert.equal(code, 0, stderr);
  const after = await readFile(path, "utf8");
  assert.deepEqual(Object.keys(JSON.parse(after)), ["ingest", "graph"]);
  assert.deepEqual(JSON.parse(after).graph, graph);
  const key = '\n  "graph":';
  assert.equal(after.slice(after.indexOf(key)), before.slice(before.indexOf(key)));
  assert.notEqual(after, before);
});

test("a graph half of the wrong shape is refused, not overwritten (M13)", async () => {
  const path = await copy("bad-graph.json", { graph: { nodes: [], edges: "not a list" } });
  const before = await readFile(path, "utf8");
  const { code, stderr } = await writer(path);
  assert.equal(code, 2);
  assert.match(stderr, /`graph` is present and is not the shape/);
  assert.equal(await readFile(path, "utf8"), before);
});

test("an ingest half already present is refused (the pre-existing guard)", async () => {
  const path = await copy("full.json", { ingest: { collections: [] }, graph });
  const before = await readFile(path, "utf8");
  const { code, stderr } = await writer(path);
  assert.equal(code, 2);
  assert.match(stderr, /`ingest` is already present/);
  assert.equal(await readFile(path, "utf8"), before);
});

test("an unparseable fixture refuses with 2, not a SyntaxError (m33)", async () => {
  const path = await copy("broken.json", "{ this is not json");
  const { code, stderr } = await writer(path);
  assert.equal(code, 2);
  assert.match(stderr, /is not JSON/);
  assert.equal(await readFile(path, "utf8"), "{ this is not json");
});

test("a root that is not an object refuses with 2", async () => {
  const path = await copy("array.json", [1, 2, 3]);
  const { code, stderr } = await writer(path);
  assert.equal(code, 2);
  assert.match(stderr, /root is not an object/);
});

test("a file with no graph half is written whole, graph null (first run)", async () => {
  const path = join(WORK, "absent-first-run.json");
  const { code, stderr } = await writer(path);
  assert.equal(code, 0, stderr);
  const written = JSON.parse(await readFile(path, "utf8"));
  assert.deepEqual(Object.keys(written), ["ingest", "graph"]);
  assert.equal(written.graph, null);
  assert.equal(typeof written.ingest, "object");
});

test("the committed fixture is not a target: a run against it refuses with 2", async () => {
  // The generator is a deliberate act; run against the committed file it must refuse.
  const before = await readFile(FIXTURE, "utf8");
  const { code } = await writer(FIXTURE);
  assert.equal(code, 2);
  assert.equal(await readFile(FIXTURE, "utf8"), before);
});

test("an unknown flag refuses with 2", async () => {
  try {
    await run(process.execPath, ["--experimental-strip-types", WRITER, "--wat"], { cwd: join(HERE, "..") });
    assert.fail("expected a refusal");
  } catch (err) {
    assert.equal(err.code, 2);
    assert.match(err.stderr, /unknown argument '--wat'/);
  }
});