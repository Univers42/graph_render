#!/usr/bin/env node
// Reads a JSON rows file, maps it with `rowsToIngest` and calls `sync`. The rows file is this
// directory's `rows.json` unless GRAPH_HUB_ROWS says otherwise.
import { readFile } from "node:fs/promises";
import { createPlugin } from "../../../crates/graph-sdk-js/src/plugin.ts";
import { rowsToIngest } from "../../../crates/graph-sdk-js/src/adapters/rows.ts";

const rows = JSON.parse(await readFile(process.env.GRAPH_HUB_ROWS ?? new URL("rows.json", import.meta.url), "utf8"));
const source = rowsToIngest(breakOf(rows));
const plugin = createPlugin({
  baseUrl: process.env.GRAPH_HUB_URL ?? "http://127.0.0.1:8081",
  apiKey: process.env.GRAPH_HUB_KEY,
  plugin: process.env.GRAPH_HUB_PLUGIN ?? "rows-file",
  manifest: manifestOf(source),
});
await plugin.register(source.source);
const answers = await plugin.sync(source.source, source);
process.stdout.write(`${answers.length} batches\n`);

// GM_HUB_SDK_BREAK=1 is the `hub-sdk` negative control (§8): the example pushes a record that is
// not in the rows file, so the live row must go red for that reason.
function breakOf(rows) {
  return process.env.GM_HUB_SDK_BREAK === "1"
    ? { ...rows, tables: rows.tables.map((t, i) => (i === 0 ? { ...t, rows: [...t.rows, wrongRow(t)] } : t)) }
    : rows;
}

// The manifest is the ingest's own declaration, read out of `source.collections`: a field's role
// is the wire's lowercase name, and `link` is `null` for every role that is not a link.
function manifestOf(source) {
  return {
    version: 1,
    manifestVersion: 1,
    name: source.source,
    collections: source.collections.map((collection) => ({
      id: collection.id,
      name: collection.name,
      titleField: collection.titleField,
      fields: collection.fields.map((field) => ({ id: field.id, name: field.name, role: field.role, link: field.link })),
    })),
  };
}

function wrongRow(table) {
  const title = table.columns.find((column) => column.role === "title")?.name ?? "title";
  return { id: "wrong", values: { [title]: "wrong" } };
}
