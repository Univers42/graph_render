import { AnalysisRefusedError } from "./errors.ts";
import type { AnalysisResult, AnalysisValueKind } from "./types.ts";

/** The wire's `u32` domain (D6): an integer in `0..0xffffffff`, which is exactly the set of
 *  values a wasm `i32` read back `>>> 0` can hold. `isU32` is the single statement of that
 *  domain; every member whose contract type is `u32` — a labelling, a depth level — is
 *  checked with it, so none of them can drift into "any number". */
function isU32(value: unknown): value is number {
  return (
    typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= 0xffffffff
  );
}

/** One optional member of an analysis face, when it is present and of the right type.
 *  An absent member is `undefined`; a member of the *wrong* type is a refusal, never a
 *  silent `undefined` — a caller told `converged: undefined` would read it as "no flag
 *  was handed back" and trust numbers that were never verified.
 */
function optionalBoolean(face: Record<string, unknown>, key: string, asked: string): boolean | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (typeof value !== "boolean") throw new AnalysisRefusedError(`${asked}: ${key} is not a boolean`);
  return value;
}

/** As {@link optionalBoolean}, for `modularity`: a score, so any finite number, negative
 *  included. It is not a count and has no lower bound. */
function optionalNumber(face: Record<string, unknown>, key: string, asked: string): number | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new AnalysisRefusedError(`${asked}: ${key} is not a finite number`);
  }
  return value;
}

/** As {@link optionalBoolean}, for `max`, which `docs/contract/wasm-abi.md` "ANALYSIS" calls
 *  the deepest hierarchy level reached — a `u32` depth level, not a number. `"max": -1`
 *  would otherwise reach a caller as a depth level that does not exist. */
function optionalU32(face: Record<string, unknown>, key: string, asked: string): number | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (!isU32(value)) {
    throw new AnalysisRefusedError(`${asked}: ${key} is not a u32 (an integer in 0..0xffffffff)`);
  }
  return value;
}

/** The spec's "keys in ascending order" (`docs/contract/wasm-abi.md` "ANALYSIS"), which is
 *  what makes two runs of the same face byte-comparable (D7). Only the order is checked; no
 *  particular set of members is required, so a future analysis may add one.
 *
 *  Compared as UTF-8 bytes, matching the contract's writer and `src/adapters/cells.ts`'s
 *  `compareBytes`, *not* `Array.prototype.sort`'s default, which is UTF-16 code-unit order
 *  and disagrees with byte order outside ASCII. `Object.keys` gives insertion order, and
 *  `JSON.parse` builds the object in the text's order, so this sees the wire's order. */
function requireAscendingKeys(face: Record<string, unknown>, asked: string): void {
  const keys = Object.keys(face);
  for (const [index, current] of keys.entries()) {
    const previous = keys[index - 1];
    if (previous === undefined || previous === current) continue;
    const order = Buffer.compare(Buffer.from(previous, "utf8"), Buffer.from(current, "utf8"));
    if (order >= 0) {
      throw new AnalysisRefusedError(
        `${asked}: keys are not in ascending byte order, "${previous}" came before "${current}" ` +
          "(docs/contract/wasm-abi.md ANALYSIS, so two runs stay byte-comparable; D7)",
      );
    }
  }
}

/** `values` must agree with the face's own declared `kind`: a `u32` labelling carrying
 *  `-1`, `1.5` or `4294967296` is a face that does not agree with itself, and an `f64` one
 *  carrying a non-finite number is no better (`1e999` is valid JSON text and parses to
 *  `Infinity`, so `JSON.parse` will not refuse it for us). */
function requireValuesAgreeWithKind(values: number[], kind: AnalysisValueKind, asked: string): void {
  if (kind === "u32") {
    if (!values.every(isU32)) {
      throw new AnalysisRefusedError(`${asked}: values must be u32 (integers in 0..0xffffffff) for kind "u32"`);
    }
    return;
  }
  if (!values.every(Number.isFinite)) {
    throw new AnalysisRefusedError(`${asked}: values must be finite f64 for kind "f64"`);
  }
}

/** The ABI's canonical JSON face (`docs/contract/wasm-abi.md` "ANALYSIS"), parsed into
 *  {@link AnalysisResult} and **checked member by member rather than cast**. A face whose
 *  `id`, `kind`, `nodeCount`, `values` or key order did not agree with itself would otherwise
 *  reach a caller as a plausible-looking object holding someone else's numbers, which is
 *  the one failure mode a typed wrapper exists to prevent — so each disagreement is an
 *  {@link AnalysisRefusedError} naming what did not match.
 */
export function parseAnalysisFace(text: string, asked: string): AnalysisResult {
  const parsed: unknown = JSON.parse(text);
  if (typeof parsed !== "object" || parsed === null) {
    throw new AnalysisRefusedError(`${asked}: the result is not a JSON object`);
  }
  const face = parsed as Record<string, unknown>;
  requireAscendingKeys(face, asked);
  const { id, kind, nodeCount, values } = face;
  if (id !== asked) {
    throw new AnalysisRefusedError(`${asked}: the motor answered for "${String(id)}"`);
  }
  if (kind !== "f64" && kind !== "u32") {
    throw new AnalysisRefusedError(`${asked}: unknown value kind ${JSON.stringify(kind)}`);
  }
  if (!Array.isArray(values) || values.some((v) => typeof v !== "number")) {
    throw new AnalysisRefusedError(`${asked}: values is not an array of numbers`);
  }
  const numbers = values as number[];
  if (nodeCount !== numbers.length) {
    throw new AnalysisRefusedError(`${asked}: nodeCount ${String(nodeCount)} but ${numbers.length} values`);
  }
  requireValuesAgreeWithKind(numbers, kind, asked);
  const typedKind: AnalysisValueKind = kind;
  return {
    id,
    kind: typedKind,
    nodeCount: numbers.length,
    values: numbers,
    converged: optionalBoolean(face, "converged", asked),
    modularity: optionalNumber(face, "modularity", asked),
    max: optionalU32(face, "max", asked),
  };
}
