import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

const EXAMPLE = "examples/plugins/rows-file/sync.mjs";
const KEY = "hub-key-1";
const PLUGIN = "rows-file";

// The child gets its `fetch` from a module this test writes and `--import`s, so the example runs
// as a real process against a fake and nothing listens on a socket. The fake answers the three
// plugin routes and appends every batch body to `GM_FAKE_BATCH_LOG`.
const FAKE = `globalThis.fetch = async (url, init) => {
  const body = init?.body === undefined ? null : JSON.parse(init.body);
  const reply = (value, status) => new Response(JSON.stringify(value), {
    status, headers: { "Content-Type": "application/json" },
  });
  if (url.includes("/batches")) {
    appendFileSync(process.env.GM_FAKE_BATCH_LOG, JSON.stringify({
      body,
      authorization: init?.headers?.["Authorization"] ?? null,
      idempotencyKey: init?.headers?.["Idempotency-Key"] ?? null,
      ifMatch: init?.headers?.["If-Match"] ?? null,
    }) + "\\n");
    return reply({ seq: 1, applied: 1 });
  }
  if (url.includes("/records")) return reply({ plugin_seq: "1.0", records: [] });
  return reply(body ?? { version: 1 }, 201);
};
`;

// `import { appendFileSync }` has to be in the fake's own scope, not the test's.
const FAKE_FULL = `import { appendFileSync } from "node:fs";\n${FAKE}`;

/** Runs `sync.mjs` as a child process and answers what the fake hub saw. */
function runSync({ rows, break: breakMode = "clear" } = {}) {
  const dir = "target/hub-example";
  mkdirSync(dir, { recursive: true });
  const fake = `${dir}/fake-hub.mjs`;
  const log = `${dir}/batches.ndjson`;
  writeFileSync(fake, FAKE_FULL);
  writeFileSync(log, "");
  const env = {
    ...process.env,
    GRAPH_HUB_URL: "http://hub.invalid:8081",
    GRAPH_HUB_KEY: KEY,
    GRAPH_HUB_PLUGIN: PLUGIN,
    // `./` is required: a bare relative specifier on `--import` is read as a package name.
    GM_FAKE_BATCH_LOG: log,
  };
  // `node --test` propagates this process's own GM_HUB_SDK_BREAK, which is how the
  // `negctl-hub-sdk-example` row turns this file red. So the break is chosen per call:
  // "clear" and "on" pin the child either way, and "inherit" is what the negative control needs.
  if (breakMode === "clear") delete env.GM_HUB_SDK_BREAK;
  else if (breakMode === "on") env.GM_HUB_SDK_BREAK = "1";
  else env.GM_HUB_SDK_BREAK = process.env.GM_HUB_SDK_BREAK;
  if (rows !== undefined) env.GRAPH_HUB_ROWS = rows;
  const child = spawnSync(process.execPath, ["--experimental-strip-types", "--import", `./${fake}`, EXAMPLE], {
    env,
    encoding: "utf8",
  });
  const seen = readFileSync(log, "utf8")
    .split("\n")
    .filter((line) => line !== "")
    .map((line) => JSON.parse(line));
  return { child, seen, stdout: child.stdout, stderr: child.stderr };
}

test("hub_example_exits_zero_on_the_rows_file", () => {
  const { child, seen, stdout } = runSync();
  assert.equal(child.status, 0, `sync.mjs failed: ${child.stderr}`);
  assert.match(stdout, /^1 batches\n$/, "one batch for three rows, well under GRAPH_HUB_MAX_BATCH");
  assert.equal(seen.length, 1);
  assert.equal(seen[0].authorization, `Bearer ${KEY}`, "the key goes in the header, never the URL");
  assert.ok(seen[0].idempotencyKey, "every batch carries an Idempotency-Key");
  assert.equal(seen[0].ifMatch, '"1.0"', "If-Match is the plugin's own plugin_seq, quoted");
});

test("hub_example_upserts_the_live_rows_and_nothing_else", () => {
  const { seen } = runSync();
  // Two live rows become upserts; the `deleted: true` row is not stored, so §7's N14 drops it
  // rather than sending a delete for an id the hub never had.
  assert.deepEqual(seen[0].body.deletes, []);
  assert.deepEqual(seen[0].body.upserts.map((up) => up.id), ["1", "2"]);
  assert.deepEqual(seen[0].body.upserts[0].values, { state: "open", title: "Crash on load" });
  // Canonical order: `deletes` before `upserts`, and a record's own keys sorted by bytes.
  assert.deepEqual(Object.keys(seen[0].body), ["deletes", "upserts"]);
  assert.deepEqual(Object.keys(seen[0].body.upserts[0]), ["collection", "id", "updatedAt", "values"]);
});

test("hub_example_pushes_no_record_that_is_not_in_the_rows_file", () => {
  // The `hub-sdk` negative control, proved without a live hub. Under `GM_HUB_SDK_BREAK=1` the
  // example appends one record that is not in `rows.json`, and this assertion is what goes red —
  // so `negctl-hub-sdk-example` greps this line's message and not merely a non-zero exit.
  const { child, seen } = runSync({ break: "inherit" });
  assert.equal(child.status, 0, `sync.mjs failed: ${child.stderr}`);
  const pushed = seen.flatMap((call) => call.body.upserts).filter((up) => up.id === "wrong");
  assert.equal(pushed.length, 0, `the example pushed a record that is not in the rows file: ${JSON.stringify(pushed)}`);
});

test("hub_example_break_pushes_the_record_that_is_not_in_the_rows_file", () => {
  // The other half of the same control: with the break on, the wrong record really is sent, so a
  // red `negctl-hub-sdk-example` is the example misbehaving and not the fake misanswering.
  const { child, seen } = runSync({ break: "on" });
  assert.equal(child.status, 0, `sync.mjs failed: ${child.stderr}`);
  const pushed = seen.flatMap((call) => call.body.upserts).filter((up) => up.id === "wrong");
  assert.equal(pushed.length, 1, "the break appends exactly one wrong record");
  assert.equal(pushed[0].values.title, "wrong");
});

test("hub_example_reads_the_three_environment_names", () => {
  // A rows file of its own, with a collection the example's own does not have: if
  // `GRAPH_HUB_ROWS` were ignored, the batch would name `issue` and not `note`.
  writeFileSync(
    "target/hub-example/other.json",
    JSON.stringify({
      source: "ops",
      tables: [
        {
          id: "note",
          titleColumn: "title",
          columns: [{ name: "title", role: "title" }],
          rows: [{ id: "n1", values: { title: "A note" } }],
        },
      ],
    }),
  );
  const { child, seen } = runSync({ rows: "target/hub-example/other.json" });
  assert.equal(child.status, 0, `GRAPH_HUB_ROWS was not read: ${child.stderr}`);
  assert.deepEqual(seen[0].body.upserts.map((up) => [up.collection, up.id]), [["note", "n1"]]);
});

test("hub_example_fails_loudly_on_a_rows_file_it_cannot_map", () => {
  writeFileSync("target/hub-example/bad.json", JSON.stringify({ source: "ops", tables: [{ id: "x" }] }));
  const { child } = runSync({ rows: "target/hub-example/bad.json" });
  assert.notEqual(child.status, 0, "a malformed rows file is not a silent success");
  assert.match(child.stderr, /titleColumn|rows|column/i);
});
