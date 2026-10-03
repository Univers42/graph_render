// `compareBytes` is UTF-8 byte order without Node's `Buffer`, which the studio's browser
// worker lacks (`Buffer is not defined` in `scripts/studio-filters.sh`, 2026-10-03). Node's
// own `Buffer.compare` is the reference here, over every pair of a set chosen to sit on the
// boundaries where code units and bytes disagree.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/byte-order.test.mjs

import assert from "node:assert/strict";
import { readFileSync, readdirSync } from "node:fs";
import { test } from "node:test";

import { compareBytes } from "../src/adapters/cells.ts";

const SAMPLES = [
  "", "a", "ab", "b", "Z", "é", "ÿ", "ࠀ", "퟿", "", "�", "￿",
  "\u{10000}", "\u{1F600}", "\u{10FFFF}", "a\u{1F600}", "a￿", "\uD800", "\uDC00",
  "\uD800a", "a\uDBFF", "\u{1F600}\uD800", "id-10", "id-9",
];

test("compareBytes agrees with Buffer.compare on every pair", () => {
  for (const a of SAMPLES) {
    for (const b of SAMPLES) {
      const expected = Buffer.compare(Buffer.from(a, "utf8"), Buffer.from(b, "utf8"));
      assert.equal(compareBytes(a, b), expected, `${JSON.stringify(a)} vs ${JSON.stringify(b)}`);
    }
  }
});

test("no SDK source calls Node's Buffer", () => {
  const src = new URL("../src/", import.meta.url);
  const files = readdirSync(src, { recursive: true }).filter((name) => String(name).endsWith(".ts"));
  assert.ok(files.length > 10, "the scan found the sources");
  for (const file of files) {
    const text = readFileSync(new URL(String(file), src), "utf8");
    assert.doesNotMatch(text, /\bBuffer\./, file);
  }
});
