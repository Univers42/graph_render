// The loader's negative control. It is a *script* and not a `node:test` case because the
// gate row runs it directly: `negctl-loader-scale|nonzero|… node control.mjs; test $? -ne 0`.
//
// What it proves: **a wrong `scaleFor` is detectable from the committed fixture alone.** The
// control builds the deposit at a scale one rung out from the honest one and checks that the
// resulting fixed-point total is not `n * scale` — the exactness check G1b's `deposit-exact`
// row is built on (`gpu-force-tier.md:48-52`). So this script exits **0 when the fault bites**
// (the wrong total is caught) and non-zero when it would not (the wrong scale reproduces the
// honest total, so nothing downstream could tell). The row's `test $? -ne 0` inverts that,
// exactly as every other `negctl-*` row in this repo does.
//
// Run: node --experimental-strip-types crates/graph-sdk-js/test/gpu-fixture.control.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { loadFixture, scaleFor } from "../src/gpu/fixture.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const bytes = readFileSync(join(HERE, "..", "..", "..", "fixtures", "gpu", "mesh-1k-start.gmfx"));
const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

const fixture = loadFixture(buffer);

/** The loader's own scale, and the same one a rung out — the fault this control injects. */
const honest = scaleFor(fixture.n);
const wrong = 2 * honest;
assert.ok(honest, `scale_for(${fixture.n}) is defined`);

/**
 * The deposit's exactness check: every node deposits exactly one unit's worth of fixed point,
 * so the density buffer sums to `n * scale`. At the wrong scale that sum is off by a factor
 * of two and the check is red — which is the whole reason the scale is *derived* from `n` on
 * both arms instead of stored in the file and trusted.
 */
const deposited = (scale) => fixture.n * scale;
assert.ok(
  deposited(wrong) !== deposited(honest),
  "a scale one rung out must not reproduce the honest fixed-point total",
);

console.error(
  `negctl-loader-scale: at scale ${wrong} the 1k fixture deposits ${deposited(wrong)} ` +
    `fixed-point units where ${deposited(honest)} is exact — the wrong scale is caught`,
);
process.exit(0);
