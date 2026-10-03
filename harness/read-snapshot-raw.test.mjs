// Behaviour the gate row cannot show on its own: the refusals, the mutations, and the
// one large snapshot that used to take the process down. Each case runs the arm as a
// child process, because what is being pinned is its exit code and its last line.
//
// The producer's own artifact is the gate row's business (`snapshot-raw` in
// `scripts/orch/rows/develop-full.rows`); the snapshot below is a small valid document
// written here, so these tests stay hermetic and do not need a built binary.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const run = promisify(execFile);
const ARM = join(dirname(fileURLToPath(import.meta.url)), "read-snapshot-raw.mjs");
const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const SCHEMA = join(ROOT, "docs", "contract", "snapshot-schema.json");
const WORK = await mkdtemp(join(tmpdir(), "read-snapshot-raw-"));

/** Four nodes, five edges: the smallest document the arm accepts. */
const VALID = {
  edges: {
    id: ["e0", "e1"],
    source: ["n0", "n1"],
    target: ["n1", "n0"],
  },
  geometry: { edges: { kind: "Line" }, nodes: { kind: "Point", x: [-0.5, 0.5], y: [-0.5, 0.5] } },
  nodes: { id: ["n0", "n1"] },
  notes: { code: [], index: [] },
  version: { major: 0, minor: 3 },
};

/** The arm, as `{ code, stdout, stderr }`: the exit code is the assertion, never a throw. */
async function arm(...args) {
  try {
    const { stdout, stderr } = await run(process.execPath, ["--experimental-strip-types", ARM, ...args], {
      cwd: ROOT,
      maxBuffer: 256 * 1024 * 1024,
    });
    return { code: 0, stdout, stderr };
  } catch (err) {
    return { code: err.code, stdout: err.stdout ?? "", stderr: err.stderr ?? "" };
  }
}

const write = async (name, value) => {
  const path = join(WORK, name);
  await writeFile(path, typeof value === "string" ? value : JSON.stringify(value));
  return path;
};

/** A copy of the committed schema with one `$def` branch mutated. */
async function schemaWith(mutate) {
  const schema = JSON.parse(await readFile(SCHEMA, "utf8"));
  mutate(schema);
  return write(`schema-${Math.abs(hash(JSON.stringify(schema)))}.json`, schema);
}

function hash(text) {
  let h = 0;
  for (let i = 0; i < text.length; i += 1) h = (h * 31 + text.charCodeAt(i)) | 0;
  return h;
}

test("no snapshot and no --selftest refuses with usage, exit 2", async () => {
  const { code, stdout, stderr } = await arm();
  assert.equal(code, 2);
  assert.match(stderr, /no snapshot given/);
  assert.match(stderr, /usage: read-snapshot-raw\.mjs <snapshot\.json>/);
  assert.equal(stdout, "");
});

test("--selftest reads the worked example and passes", async () => {
  const { code, stdout } = await arm("--selftest");
  assert.equal(code, 0);
  assert.match(stdout, /# pass/);
});

test("an unknown flag refuses with usage, exit 2", async () => {
  const { code, stderr } = await arm("--wat", join(WORK, "valid.json"));
  assert.equal(code, 2);
  assert.match(stderr, /unknown argument '--wat'/);
});

test("a document the arm accepts passes", async () => {
  const path = await write("valid.json", VALID);
  const { code, stdout } = await arm(path);
  assert.equal(code, 0, stdout);
  assert.match(stdout, /# 2 nodes, 2 edges, Point nodes \/ Line edges/);
  assert.match(stdout, /bounds x\[-0\.5, 0\.5\] y\[-0\.5, 0\.5\]/);
  assert.match(stdout, /# pass/);
});

test("a snapshot missing a member fails naming the member, exit 1 (M11)", async () => {
  const broken = structuredClone(VALID);
  delete broken.geometry;
  const { code, stdout } = await arm(await write("no-geometry.json", broken));
  assert.equal(code, 1);
  assert.match(stdout, /not ok - the snapshot carries every member the schema requires/);
  assert.match(stdout, /snapshot\.geometry: missing nodes/);
  assert.doesNotMatch(stdout, /TypeError/);
});

test("an open oneOf branch in the schema fails naming the def and the kind (M12)", async () => {
  const schema = await schemaWith((s) => {
    const line = s.$defs.EdgeGeometry.oneOf.find((b) => b.properties.kind.const === "Line");
    delete line.additionalProperties;
  });
  const { code, stdout } = await arm("--schema", schema, await write("valid2.json", VALID));
  assert.equal(code, 1);
  assert.match(stdout, /not ok - every object in the committed schema refuses an unknown member/);
  assert.match(stdout, /#\s+#\/\$defs\/EdgeGeometry\/oneOf\[0\]\/Line/);
});

test("the committed schema itself is closed, branches included", async () => {
  const { code, stdout } = await arm(await write("valid3.json", VALID));
  assert.equal(code, 0);
  assert.match(stdout, /ok - every object in the committed schema refuses an unknown member/);
});

test("a def the walk cannot resolve is a hole, not a pass (U2)", async () => {
  const schema = await schemaWith((s) => {
    // Renamed *with* its $ref, so nothing dangles: only `shapeOf` can notice.
    s.$defs.NodeIds = s.$defs.Nodes;
    delete s.$defs.Nodes;
    s.properties.nodes.$ref = "#/$defs/NodeIds";
  });
  const { code, stdout } = await arm("--schema", schema, await write("valid4.json", VALID));
  assert.equal(code, 1);
  assert.match(stdout, /snapshot\.nodes: the schema states no object shape for #\/\$defs\/Nodes/);
});

test("a 200k-node snapshot summarises instead of overflowing the stack (m34)", async () => {
  const N = 200_000;
  const id = [];
  const x = [];
  const y = [];
  for (let i = 0; i < N; i += 1) {
    id.push(`n:${i}`);
    x.push(((i * 37) % 1009) / 1009);
    y.push(((i * 53) % 1013) / 1013);
  }
  const big = {
    ...VALID,
    edges: { id: ["e0", "e1"], source: ["n:0", "n:1"], target: ["n:1", "n:0"] },
    nodes: { id },
    geometry: { ...VALID.geometry, nodes: { kind: "Point", x, y } },
  };
  const { code, stdout, stderr } = await arm(await write("big.json", big));
  assert.equal(code, 0, stderr);
  assert.doesNotMatch(stdout + stderr, /RangeError/);
  assert.match(stdout, new RegExp(`# ${N} nodes, 2 edges, Point nodes / Line edges`));
  assert.match(stdout, /# pass\n$/);
  assert.equal(stdout.split("\n").filter((l) => l.startsWith("#   n:")).length, N);
});

test("an unknown node kind is one recorded failure, not a TypeError (m35)", async () => {
  const odd = structuredClone(VALID);
  odd.geometry.nodes.kind = "Hyperboloid";
  const { code, stdout, stderr } = await arm(await write("unknown-kind.json", odd));
  assert.equal(code, 1);
  assert.match(stdout, /not ok - the node geometry is a kind this reader can place/);
  assert.match(stdout, /# 1 failed\n$/);
  assert.doesNotMatch(stdout + stderr, /TypeError/);
});

test("a missing or unparseable schema refuses with exit 2, not a stack (m36)", async () => {
  const missing = await arm("--schema", join(WORK, "absent.json"), await write("valid5.json", VALID));
  assert.equal(missing.code, 2);
  assert.match(missing.stderr, /could not read schema .*ENOENT/);
  assert.doesNotMatch(missing.stderr, /\n {4}at /);

  const broken = await arm("--schema", await write("bad.json", "not json"), await write("valid6.json", VALID));
  assert.equal(broken.code, 2);
  assert.match(broken.stderr, /could not read schema .*not valid JSON/);
});

test("a missing snapshot refuses with exit 2", async () => {
  const { code, stderr } = await arm(join(WORK, "absent.json"));
  assert.equal(code, 2);
  assert.match(stderr, /could not read snapshot .*ENOENT/);
});