// What every live test needs from the hub `scripts/orch/hub-live.sh` started: its URL, the two
// keys (read from the files hub-live wrote and never printed), a fresh workspace per test, and
// the reads a test asserts on. Run from the repository root, which is where node-slim puts /w.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import { createPlugin } from "../../src/plugin.ts";
import { rowsToIngest } from "../../src/adapters/rows.ts";

const read = (name) => readFileSync(`target/hub-run/${name}`, "utf8").trim();

export const hubUrl = read("url");
export const adminKey = read("key");
export const writerKey = read("writer-key");
export const PLUGIN = "rows-file";

let made = 0;

/** A workspace no other test has touched: `${prefix}-${pid}-${n}`, created by the admin key. */
export async function workspace(prefix) {
  made += 1;
  const id = `${prefix}-${process.pid}-${made}`;
  await createWorkspace(id);
  return id;
}

/** `PUT /v1/workspaces/{id}`: 201 on create, 200 when it is already there. */
export async function createWorkspace(id) {
  const answer = await fetch(`${hubUrl}/v1/workspaces/${id}`, { method: "PUT", headers: auth(adminKey) });
  assert.ok(answer.status === 201 || answer.status === 200, `PUT workspace ${id}: ${answer.status}`);
}

/** `GET /graph`: the document and its ETag, quotes stripped. */
export async function graphOf(ws) {
  const answer = await fetch(`${hubUrl}/v1/workspaces/${ws}/graph`, { headers: auth(adminKey) });
  assert.equal(answer.status, 200, `GET graph ${ws}`);
  return { etag: answer.headers.get("etag").replace(/^"|"$/g, ""), doc: await answer.json() };
}

/** The ids `/graph` shows, sorted. */
export async function idsOf(ws) {
  const { doc } = await graphOf(ws);
  return doc.records.map((record) => record.id).sort();
}

/** Every change of the workspace's current epoch, from its first. */
export async function changesOf(ws) {
  const { etag } = await graphOf(ws);
  const epoch = etag.split(".")[0];
  const answer = await fetch(`${hubUrl}/v1/workspaces/${ws}/changes?since=${epoch}.0&limit=1000`, {
    headers: auth(adminKey),
  });
  assert.equal(answer.status, 200, `GET changes ${ws}`);
  return (await answer.json()).changes;
}

/** One `issue` table of rows `[{id, values, deleted?}]`, plus any extra declared columns. */
export function issues(rows, extraColumns = []) {
  return rowsToIngest({
    source: "ops",
    tables: [
      {
        id: "issue",
        name: "Issues",
        titleColumn: "title",
        columns: [
          { name: "title", label: "Title", role: "title" },
          { name: "state", label: "State", role: "scalar" },
          ...extraColumns,
        ],
        rows,
      },
    ],
  });
}

/** The example's manifest (`examples/plugins/rows-file/sync.mjs`): the ingest's own declaration. */
export function manifestOf(ingest) {
  return {
    version: 1,
    manifestVersion: 1,
    name: ingest.source,
    collections: ingest.collections.map((collection) => ({
      id: collection.id,
      name: collection.name,
      titleField: collection.titleField,
      fields: collection.fields.map((field) => ({ id: field.id, name: field.name, role: field.role, link: field.link })),
    })),
  };
}

/** The `rows-file` writer for `ingest`'s manifest, under the writer key unless told otherwise. */
export function pluginFor(ingest, options = {}) {
  return createPlugin({ baseUrl: hubUrl, apiKey: writerKey, plugin: PLUGIN, manifest: manifestOf(ingest), ...options });
}

export function auth(key) {
  return { Authorization: `Bearer ${key}` };
}
