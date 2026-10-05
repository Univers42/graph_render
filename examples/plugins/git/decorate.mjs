#!/usr/bin/env node
// decorate.mjs <ingest.json> <graph.json> <field> <out>: the derived graph with every record's
// `<field>` cell copied onto its node as `tags`, the member the studio's `tag:#x` query reads
// (packages/graph-studio/src/source/ingest.ts). It knows the contract's node id rule
// (`source:collection:record`, crates/graph-core/src/ingest/build.rs) and nothing about where
// the records came from. Exit 2 when a record has no node or a cell is not a list of strings.
import { readFile, writeFile } from "node:fs/promises";

const [ingestPath, graphPath, field, outPath] = process.argv.slice(2);
if (!ingestPath || !graphPath || !field || !outPath) {
  process.stderr.write("usage: decorate.mjs <ingest.json> <graph.json> <field> <out>\n");
  process.exit(2);
}
const { ingest } = JSON.parse(await readFile(ingestPath, "utf8"));
const graph = JSON.parse(canonical(await readFile(graphPath, "utf8")));
const tags = tagsById(ingest, field);
const nodes = graph.nodes.map((node) => (tags.has(node.id) ? { ...node, tags: take(tags, node.id) } : node));
if (tags.size !== 0) refuse(`${tags.size} records have no node, first ${tags.keys().next().value}`);
await writeFile(outPath, JSON.stringify({ version: 1, nodes, edges: graph.edges }));

function tagsById(doc, name) {
  const byId = new Map();
  for (const record of doc.records) {
    const cell = record.values[name];
    if (record.deleted || cell === undefined) continue;
    if (!Array.isArray(cell) || !cell.every((t) => typeof t === "string")) {
      refuse(`record ${record.id}: \`${name}\` is not a list of strings`);
    }
    byId.set(`${doc.source}:${record.collection}:${record.id}`, cell);
  }
  return byId;
}

function take(map, id) {
  const value = map.get(id);
  map.delete(id);
  return value;
}

// `graph-cli ingest --out` writes the canonical JSON and then the readable `describe`
// rendering (`crates/graph-cli/src/ingest_cmd.rs`), so the file is not one JSON document.
// Caveat: this keeps the text before the first newline, which is the canonical JSON because
// JSON escapes every newline inside a string; the readable rendering is then dropped. A file
// whose first line is not JSON is refused by `JSON.parse`, naming no path.
function canonical(text) {
  return text.slice(0, text.indexOf("\n") === -1 ? text.length : text.indexOf("\n"));
}

function refuse(why) {
  process.stderr.write(`decorate: ${why}\n`);
  process.exit(2);
}