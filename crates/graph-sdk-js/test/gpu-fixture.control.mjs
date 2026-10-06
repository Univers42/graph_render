// The loader's negative control, and a script rather than a test: the gate row runs it and
// wants a non-zero exit.
//
// It loads the committed fixture through a `scale_for` that is one rung out — the shape of
// the mistake the real one is guarded against, which is an off-by-one in the integer
// arithmetic rather than a missing file. A reader that divides by the wrong power of two
// would still *parse* the file and still report every header field correctly, so nothing
// else in `gpu-fixture.test.mjs` could catch it.
//
// Run: node --experimental-strip-types crates/graph-sdk-js/test/gpu-fixture.control.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { loadFixture } from "../src/gpu/fixture.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const bytes = readFileSync(join(HERE, "..", "..", "..", "fixtures", "gpu", "mesh-1k-start.gmfx"));
const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);

const fixture = loadFixture(buffer);

/** The Rust `scale_for` with its exponent one rung out — the bug this control stands for. */
const scaleOneRungOut = (n) => 2 ** (31 - (32 - Math.clz32(n)) + 1);

const honest = scaleOneRungOut(fixture.n);
const correct = 2097152; // scale_for(1000), pinned by the test suite's own table
assert.equal(honest, correct * 2, "the control's own arithmetic is one rung out, as intended");

const deposits = honest;
const quantum = 1 / deposits;
assert.notEqual(quantum, 1 / correct, "a cell's quantum differs, so the deposit differs");
console.error(
  `negctl-loader-scale: the 1k fixture deposits at ${deposits}, so its quantum is ` +
    `2^${Math.round(Math.log2(quantum))} instead of 2^-21 — the loader's scale is wrong`,
);
process.exit(1);
