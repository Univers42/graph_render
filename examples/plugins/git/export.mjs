#!/usr/bin/env node
// export.mjs <name> <log> <out>: a `git log` file (map.mjs FORMAT) to `{"ingest": <contract>}`,
// the member `graph-cli ingest --member ingest` reads. Counts go to stderr.
// Exit 0 written; 2 refused, naming the log line or the adapter's path.
import { readFile, writeFile } from "node:fs/promises";
import { RowsAdapterError, rowsToIngest } from "../../../crates/graph-sdk-js/src/adapters/rows.ts";
import { LogRefusal, parseLog, toRows } from "./map.mjs";

const NAME = /^[A-Za-z0-9._-]{1,64}$/;
const [name, logPath, outPath] = process.argv.slice(2);
if (!NAME.test(name ?? "") || !logPath || !outPath) {
  process.stderr.write("usage: export.mjs <name: [A-Za-z0-9._-]{1,64}> <log> <out>\n");
  process.exit(2);
}
try {
  const commits = parseLog(await readFile(logPath, "utf8"));
  const { rows, dropped } = toRows(commits, name);
  const text = JSON.stringify({ ingest: rowsToIngest(rows) });
  await writeFile(outPath, text);
  const bytes = Buffer.byteLength(text);
  process.stderr.write(`${name}: ${commits.length} commits, ${dropped} parents outside the log dropped, ${bytes} bytes\n`);
} catch (error) {
  if (!(error instanceof LogRefusal || error instanceof RowsAdapterError)) throw error;
  process.stderr.write(`${logPath}: ${error.message}\n`);
  process.exit(2);
}