// The strings the chrome prints: an id without its family, a digest that fits, a duration
// that does not lie about being zero.
import assert from "node:assert/strict";
import { test } from "node:test";

import { digest8, ms, shortName, sig3 } from "../src/ui/names.ts";

test("an id loses its first dotted segment, and one without a dot is kept", () => {
  assert.equal(shortName("layout.force.barnes_hut"), "force.barnes_hut");
  assert.equal(shortName("off"), "off");
  assert.equal(shortName("layout.grid"), "grid");
  assert.equal(shortName(""), "");
});

test("a digest shows its first eight characters, and nothing at all is a dash", () => {
  assert.equal(digest8("0123456789abcdef"), "01234567");
  assert.equal(digest8("short"), "short");
  assert.equal(digest8(null), "—");
});

test("a duration reads as milliseconds, or as seconds once there are a thousand of them", () => {
  assert.equal(ms(0), "<1 ms");
  assert.equal(ms(0.4), "<1 ms");
  assert.equal(ms(1), "1 ms");
  assert.equal(ms(12.4), "12 ms");
  assert.equal(ms(999), "999 ms");
  assert.equal(ms(1000), "1.0 s");
  assert.equal(ms(1340), "1.3 s");
  assert.equal(ms(61230), "61.2 s");
});

test("a value shows three significant digits and no more", () => {
  assert.equal(sig3(0.5), "0.5");
  assert.equal(sig3(1), "1");
  assert.equal(sig3(1.23456), "1.23");
  assert.equal(sig3(123456), "123000");
  assert.equal(sig3(0.000123456), "0.000123");
});
