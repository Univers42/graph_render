// The analysis face's own checks (`src/analysis-face.ts`): a face that does not agree with
// itself — a `u32` labelling carrying a negative or fractional label, a `max` that is not a
// depth level, a producer that reordered the keys — must be refused by name, never handed to
// a caller as a plausible-looking object.
//
// The faces below list their keys in ascending byte order, as
// `docs/contract/wasm-abi.md` "ANALYSIS" requires, so each case differs from the contract's
// own example in exactly the member it is about.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";
import { parseAnalysisFace } from "../src/analysis-face.ts";
import { AnalysisRefusedError } from "../src/errors.ts";

const ID = "analysis.depth.bfs";

/** The contract's face with one member substituted. Keys arrive already ordered: `Object`
 *  preserves insertion order for these names, so the list *is* the order on the wire. */
const face = (pairs) => JSON.stringify(Object.fromEntries(pairs));

/** A `u32` face: keys `id`, `kind`, `nodeCount`, `values`. */
const u32Face = (values, extra = []) =>
  face([["id", ID], ["kind", "u32"], ...extra, ["nodeCount", values.length], ["values", values]]);

/** Asserts the parser refuses, and that the message says why (which is the whole point of
 *  a refusal: the caller must learn *which* member did not agree with itself). */
function refuses(text, pattern) {
  const error = (() => {
    try {
      parseAnalysisFace(text, ID);
    } catch (caught) {
      return caught;
    }
    return undefined;
  })();
  assert.ok(error !== undefined, `the face parsed instead of being refused: ${text}`);
  assert.ok(error instanceof AnalysisRefusedError, `${error.name}: ${error.message}`);
  assert.match(error.message, pattern, error.message);
}

// --- M5: `values` must agree with the face's own declared `kind` ---------------------

test("M5 a u32 face carrying a label past 0xffffffff is refused", () => {
  refuses(u32Face([4294967296]), /values/);
});

test("M5 a u32 face carrying a negative label is refused", () => {
  refuses(u32Face([-1]), /values/);
});

test("M5 a u32 face carrying a fractional label is refused", () => {
  refuses(u32Face([1.5]), /values/);
});

test("M5 both ends of the u32 range are accepted, and a whole graph of them", () => {
  const result = parseAnalysisFace(u32Face([0, 1, 0xffffffff]), ID);
  assert.deepEqual([...result.values], [0, 1, 0xffffffff]);
  assert.equal(result.kind, "u32");
});

test("M5 an f64 face keeps its f64 range: a fractional value is legal", () => {
  const result = parseAnalysisFace(face([["id", ID], ["kind", "f64"], ["nodeCount", 1], ["values", [1.5]]]), ID);
  assert.deepEqual([...result.values], [1.5]);
});

test("M5 an f64 face carrying a non-finite value is refused", () => {
  // `1e999` is valid JSON text and parses to Infinity; `JSON.parse` will not refuse it, so
  // the parser's own finiteness check is the only thing standing between it and a caller.
  refuses(face([["id", ID], ["kind", "f64"], ["nodeCount", 1], ["values", [1e999]]]), /values/);
});

// --- m18: `max` is a `u32` depth level, `modularity` a score --------------------------

test("m18 a negative depth level is refused, naming `max`", () => {
  refuses(u32Face([0, 1], [["max", -1]]), /\bmax\b/);
});

test("m18 a fractional depth level is refused, naming `max`", () => {
  refuses(u32Face([0, 1], [["max", 1.5]]), /\bmax\b/);
});

test("m18 a depth level past 0xffffffff is refused, naming `max`", () => {
  refuses(u32Face([0, 1], [["max", 4294967296]]), /\bmax\b/);
});

test("m18 the contract's own `max: 2` is accepted", () => {
  const result = parseAnalysisFace(u32Face([0, 1, 2], [["max", 2]]), ID);
  assert.equal(result.max, 2);
});

test("m18 `modularity` stays any finite number, including a negative score", () => {
  const result = parseAnalysisFace(
    u32Face([0, 0], [["modularity", -0.25]]),
    ID,
  );
  assert.equal(result.modularity, -0.25);
});

// --- m19: keys in ascending order (D7 byte-comparability) -----------------------------

test("m19 a reordered face is refused, naming the pair that is out of order", () => {
  // `values` before `nodeCount` is the wrong way round: "n" sorts before "v".
  refuses(
    face([["id", ID], ["kind", "u32"], ["values", [0]], ["nodeCount", 1]]),
    /ascending[\s\S]*"values"[\s\S]*"nodeCount"/,
  );
});

test("m19 the optional members are checked for order too", () => {
  // `values` before `max`: "v" is past "m", so the face no longer compares byte for byte.
  refuses(
    face([["id", ID], ["kind", "u32"], ["nodeCount", 1], ["values", [0]], ["max", 1]]),
    /ascending[\s\S]*"values"[\s\S]*"max"/,
  );
});

test("m19 a face in ascending order parses, optional members included", () => {
  const result = parseAnalysisFace(
    face([
      ["converged", false],
      ["id", ID],
      ["kind", "u32"],
      ["max", 2],
      ["modularity", 0.5],
      ["nodeCount", 1],
      ["values", [0]],
    ]),
    ID,
  );
  assert.equal(result.converged, false);
  assert.equal(result.max, 2);
  assert.equal(result.modularity, 0.5);
});

test("m19 the order is UTF-8 byte order, not a locale or UTF-16 collation", () => {
  // `Z` (0x5a) sorts before `a` (0x61) by byte and after it by nothing else we use; the
  // point is the comparison is on bytes, so a future non-ASCII member is not mis-ordered.
  refuses(
    face([["id", ID], ["kind", "u32"], ["Zeta", 1], ["nodeCount", 0], ["values", []]]),
    /ascending[\s\S]*"Zeta"/,
  );
});

// --- the refusals that were already there still are -----------------------------------

test("a face answering for another analysis is refused", () => {
  refuses(
    face([["id", "analysis.centrality.degree"], ["kind", "f64"], ["nodeCount", 0], ["values", []]]),
    /analysis\.centrality\.degree/,
  );
});

test("a `nodeCount` that disagrees with `values` is refused", () => {
  refuses(
    face([["id", ID], ["kind", "u32"], ["nodeCount", 2], ["values", [0]]]),
    /nodeCount/,
  );
});
