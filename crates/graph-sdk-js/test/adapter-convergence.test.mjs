// The adapter-convergence harness's own guards (M14, m31, and its B1/B2/M8/M9 negative
// cases, from `docs/reviews/review-harness-sdk.md`), split out of `adapters.test.mjs` by
// the house's 300-line limit. The harness runs its checks at import time, so these tests
// read its captured stdout.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/adapter-convergence.test.mjs

import assert from "node:assert/strict";
import { test } from "node:test";

let loaded = null;

/** `harness/adapter-convergence.mjs`, loaded fresh with its module-scope output captured.
 * Its checks run at import time — deliberately, so the adapter-convergence gate row runs
 * them — and that stdout is the only place a test can see them. */
async function convergenceModule() {
  if (loaded !== null) return loaded;
  const url = new URL("../../../harness/adapter-convergence.mjs", import.meta.url).href;
  const lines = [];
  const write = process.stdout.write.bind(process.stdout);
  process.stdout.write = (chunk) => {
    lines.push(String(chunk));
    return true;
  };
  try {
    loaded = { module: await import(`${url}?once=1`), output: lines.join("") };
  } finally {
    process.stdout.write = write;
  }
  return loaded;
}

test("M14: a missing member in a committed fixture is 'could not run', not a TypeError", async () => {
  const { module } = await convergenceModule();
  // Both readers go through the guard, so a fixture missing either member is caught here.
  // `fail` exits 2; exit is caught so the refusal is observable in-process. What matters
  // is that the refusal happens *at the reader*, and not later as a `TypeError` from
  // `Math.min(len, undefined)` with the "first difference at byte N" report lost.
  const exit = process.exit;
  const refused = [];
  process.exit = (code) => {
    refused.push(code);
    throw new Error(`process.exit(${code})`);
  };
  try {
    // `async` so a synchronous `fail` surfaces as a rejection rather than escaping the
    // assertion — the guard throws by way of `process.exit`, and that is the point.
    for (const name of ["ingest", "graph"]) {
      await assert.rejects(async () => module.requiredMember({ other: {} }, name), /process\.exit\(2\)/);
    }
    // An absent member, and a present one still reads through.
    await assert.rejects(async () => module.requiredMember({}, "ingest"), /process\.exit\(2\)/);
    assert.deepEqual(module.requiredMember({ ingest: { version: 1 } }, "ingest"), { version: 1 });
  } finally {
    process.exit = exit;
  }
  assert.deepEqual(refused, [2, 2, 2], "exit 2 — 'could not run', not exit 1");
  assert.equal(typeof (await module.expectedIngest()), "object", "the committed member still reads through");
});

test("m31: every override key is one the fixture's databases declare", async () => {
  const { module, output } = await convergenceModule();
  const declared = [
    ...Object.keys(module.NOTION_OVERRIDES.roles),
    ...Object.keys(module.NOTION_OVERRIDES.links),
  ];
  for (const key of declared) {
    assert.ok(output.includes(`ok - every override key is one the fixture declares: ${key}`), `checked ${key}`);
  }
  assert.equal((output.match(/not ok/g) ?? []).length, 0, "no module-scope check fails");
});

test("B1/B2/M8/M9: the harness's own negative cases hold", async () => {
  const { module } = await convergenceModule();
  assert.ok(module.adapterRefusals.length >= 4, "the negative cases are exported");
  for (const refusal of module.adapterRefusals) {
    assert.equal(refusal.ok, true, refusal.name);
  }
});