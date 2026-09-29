// The console: what the log says, and the one line that runs something.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { LogEntry } from "../../src/state/model.ts";
import { Console } from "../../src/ui/Console.tsx";
import { DRAWN, DIGEST, markup, studioWith } from "./desk.ts";

const HINT = "The ingest document was refused. Check the JSON, or regenerate the synthetic graph.";

const RAN: LogEntry = {
  seq: 1, command: "layout layout.forceatlas2", ok: true, ms: 12.5,
  message: "layout.forceatlas2 13 ms", digest: DIGEST, notes: ["snapshots are v2"],
  error: null,
};
const FAILED: LogEntry = {
  seq: 2, command: "synthetic 400 2 1 vault", ok: false, ms: 3, message: "the layout refused",
  digest: null, notes: [],
  error: { title: "BuildRefusedError", code: "code 3 (E_BUILD)", detail: "the document is not the shape", hint: HINT },
};

function console(): string {
  const state = { ...DRAWN, log: [RAN, FAILED] };
  const { studio } = studioWith(state);
  return markup(createElement(Console, { studio, state, onClose: () => undefined }));
}

test("the log is polite, and an entry is the command, how long it took and what it said", () => {
  const html = console();
  assert.match(html, /role="log" aria-live="polite"/);
  assert.match(html, />&gt; layout layout\.forceatlas2</);
  assert.match(html, /13 ms/);
  assert.match(html, /snapshots are v2/);
});

test("a run that worked shows eight characters of its digest", () => {
  assert.match(console(), new RegExp(DIGEST.slice(0, 8)));
});

test("a run that failed says so, and says what the reader can do about it", () => {
  const html = console();
  assert.match(html, /class="gs-entry gs-failed"/);
  assert.ok(html.includes(HINT), "the hint is in the markup");
  assert.ok(html.includes("BuildRefusedError"), "and the title that names what failed");
  assert.ok(html.includes("code 3 (E_BUILD)"), "and the code it came back with");
  assert.equal(html.match(/gs-failed/g)?.length, 1, "only the failed entry is marked");
});

test("the header carries the title, a Clear and a Close", () => {
  const html = console();
  assert.match(html, />Console</);
  assert.match(html, /aria-label="Clear the log"/);
  assert.match(html, /aria-label="Close the console"/);
  assert.match(html, /aria-label="Console command"/);
});
