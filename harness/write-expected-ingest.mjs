// One-off generator for `fixtures/ingest/expected-graph.json`'s `ingest` member. Kept
// out of the harness proper: a fixture must be readable and checkable without running
// its own generator, and the check is `harness/sdk-smoke.mjs --adapter-convergence`.
//
//   node --experimental-strip-types harness/write-expected-ingest.mjs
//
// It writes only the `ingest` half, refusing if the file already has one, so a
// regeneration is always a deliberate act rather than a side effect of a test run.
import { readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { canonicalJson, ingestFromRows } from "./adapter-convergence.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const PATH = join(HERE, "..", "fixtures", "ingest", "expected-graph.json");

const ingest = await ingestFromRows();
const text = JSON.stringify(JSON.parse(canonicalJson(ingest)), null, 2);

let existing = "";
try {
  existing = await readFile(PATH, "utf8");
} catch {
  // A missing file is the normal first run.
}
if (existing.length > 0 && JSON.parse(existing).ingest !== undefined) {
  process.stderr.write("write-expected-ingest: `ingest` is already present; refusing to overwrite\n");
  process.exit(2);
}
await writeFile(PATH, `{\n  "ingest": ${text.replace(/^/gm, "  ").trimStart()},\n  "graph": null\n}\n`);
process.stdout.write(`wrote ${PATH}\n`);
